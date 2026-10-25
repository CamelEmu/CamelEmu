// Copyright 2017 Citra Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <future>
#include <memory>
#include <string>
#include "common/detached_tasks.h"
#include "common/logging/log.h"
#include "emu_json.h"
#include "web_service/announce_room_json.h"
#include "web_service/web_backend.h"

namespace AnnounceMultiplayerRoom {

namespace {

// emu_json_free is a no-op on borrowed handles, so one deleter serves both
// the owning handles built here and the borrowed ones from emu_json_get.
using JsonHandle = std::unique_ptr<EmuJson, decltype(&emu_json_free)>;

JsonHandle Borrowed(EmuJson* handle) {
    return JsonHandle{handle, &emu_json_free};
}

bool GetString(EmuJson* parent, const char* key, std::string& out) {
    const JsonHandle child = Borrowed(emu_json_get(parent, key));
    if (!child || emu_json_type(child.get()) != EMU_JSON_STRING) {
        return false;
    }
    const char* value = emu_json_string(child.get());
    if (value == nullptr) {
        return false;
    }
    out.assign(value, emu_json_string_len(child.get()));
    return true;
}

bool GetU64(EmuJson* parent, const char* key, u64& out) {
    const JsonHandle child = Borrowed(emu_json_get(parent, key));
    if (!child || emu_json_type(child.get()) != EMU_JSON_NUMBER) {
        return false;
    }
    out = emu_json_u64(child.get());
    return true;
}

bool GetBool(EmuJson* parent, const char* key, bool& out) {
    const JsonHandle child = Borrowed(emu_json_get(parent, key));
    if (!child || emu_json_type(child.get()) != EMU_JSON_BOOL) {
        return false;
    }
    out = emu_json_bool(child.get()) != 0;
    return true;
}

bool SetString(EmuJson* object, const char* key, const std::string& value) {
    return emu_json_set(object, key, emu_json_new_string(value.c_str())) == 1;
}

bool SetU64(EmuJson* object, const char* key, u64 value) {
    return emu_json_set(object, key, emu_json_new_number_u64(value)) == 1;
}

bool SetBool(EmuJson* object, const char* key, bool value) {
    return emu_json_set(object, key, emu_json_new_bool(value ? 1 : 0)) == 1;
}

std::string Stringify(EmuJson* handle) {
    char* text = emu_json_stringify(handle, nullptr);
    if (text == nullptr) {
        return {};
    }
    std::string out{text};
    emu_json_free_string(text);
    return out;
}

EmuJson* MemberToJson(const Room::Member& member) {
    JsonHandle json(emu_json_new_object(), &emu_json_free);
    if (!member.username.empty()) {
        SetString(json.get(), "username", member.username);
    }
    SetString(json.get(), "nickname", member.nickname);
    if (!member.avatar_url.empty()) {
        SetString(json.get(), "avatarUrl", member.avatar_url);
    }
    SetString(json.get(), "gameName", member.game_name);
    SetU64(json.get(), "gameId", member.game_id);
    return json.release();
}

bool MemberFromJson(EmuJson* json, Room::Member& member) {
    if (json == nullptr || emu_json_type(json) != EMU_JSON_OBJECT) {
        return false;
    }
    u64 game_id{};
    if (!GetString(json, "nickname", member.nickname) ||
        !GetString(json, "gameName", member.game_name) || !GetU64(json, "gameId", game_id)) {
        return false;
    }
    member.game_id = game_id;
    // The username and the avatar URL authenticate the member; when either
    // is missing the member counts as unauthenticated and both are dropped.
    std::string username;
    std::string avatar_url;
    if (GetString(json, "username", username) && GetString(json, "avatarUrl", avatar_url)) {
        member.username = username;
        member.avatar_url = avatar_url;
    } else {
        member.username = member.avatar_url = "";
        LOG_DEBUG(Network, "Member \'{}\' isn't authenticated", member.nickname);
    }
    return true;
}

EmuJson* RoomToJson(const Room& room) {
    JsonHandle json(emu_json_new_object(), &emu_json_free);
    SetU64(json.get(), "port", room.port);
    SetString(json.get(), "name", room.name);
    if (!room.description.empty()) {
        SetString(json.get(), "description", room.description);
    }
    SetString(json.get(), "preferredGameName", room.preferred_game);
    SetU64(json.get(), "preferredGameId", room.preferred_game_id);
    SetU64(json.get(), "maxPlayers", room.max_player);
    SetU64(json.get(), "netVersion", room.net_version);
    SetBool(json.get(), "hasPassword", room.has_password);
    if (!room.members.empty()) {
        JsonHandle members(emu_json_new_array(), &emu_json_free);
        for (const Room::Member& member : room.members) {
            emu_json_push(members.get(), MemberToJson(member));
        }
        emu_json_set(json.get(), "players", members.release());
    }
    return json.release();
}

bool RoomFromJson(EmuJson* json, Room& room) {
    if (json == nullptr || emu_json_type(json) != EMU_JSON_OBJECT) {
        return false;
    }
    u64 port{};
    u64 max_players{};
    u64 net_version{};
    bool has_password{};
    if (!GetString(json, "externalGuid", room.verify_UID) ||
        !GetString(json, "address", room.ip) || !GetString(json, "name", room.name)) {
        return false;
    }
    if (!GetString(json, "description", room.description)) {
        room.description = "";
        LOG_DEBUG(Network, "Room \'{}\' doesn't contain a description", room.name);
    }
    if (!GetString(json, "owner", room.owner) || !GetU64(json, "port", port) ||
        !GetString(json, "preferredGameName", room.preferred_game) ||
        !GetU64(json, "preferredGameId", room.preferred_game_id) ||
        !GetU64(json, "maxPlayers", max_players) || !GetU64(json, "netVersion", net_version) ||
        !GetBool(json, "hasPassword", has_password)) {
        return false;
    }
    room.port = static_cast<u16>(port);
    room.max_player = static_cast<u32>(max_players);
    room.net_version = static_cast<u32>(net_version);
    room.has_password = has_password;

    const JsonHandle players = Borrowed(emu_json_get(json, "players"));
    if (!players) {
        LOG_DEBUG(Network, "Out of range {}", "players");
        return true;
    }
    if (emu_json_type(players.get()) != EMU_JSON_ARRAY) {
        return false;
    }
    const std::size_t count = emu_json_array_len(players.get());
    room.members.clear();
    for (std::size_t i = 0; i < count; ++i) {
        const JsonHandle element = Borrowed(emu_json_array_at(players.get(), i));
        Room::Member member;
        if (!MemberFromJson(element.get(), member)) {
            return false;
        }
        room.members.push_back(member);
    }
    return true;
}

} // namespace

} // namespace AnnounceMultiplayerRoom

namespace WebService {

void RoomJson::SetRoomInformation(const std::string& name, const std::string& description,
                                  const u16 port, const u32 max_player, const u32 net_version,
                                  const bool has_password, const std::string& preferred_game,
                                  const u64 preferred_game_id) {
    room.name = name;
    room.description = description;
    room.port = port;
    room.max_player = max_player;
    room.net_version = net_version;
    room.has_password = has_password;
    room.preferred_game = preferred_game;
    room.preferred_game_id = preferred_game_id;
}
void RoomJson::AddPlayer(const std::string& username, const std::string& nickname,
                         const std::string& avatar_url,
                         const AnnounceMultiplayerRoom::MacAddress& mac_address, const u64 game_id,
                         const std::string& game_name) {
    AnnounceMultiplayerRoom::Room::Member member;
    member.username = username;
    member.nickname = nickname;
    member.avatar_url = avatar_url;
    member.mac_address = mac_address;
    member.game_id = game_id;
    member.game_name = game_name;
    room.members.push_back(member);
}

Common::WebResult RoomJson::Update() {
    if (room_id.empty()) {
        LOG_ERROR(WebService, "Room must be registered to be updated");
        return Common::WebResult{Common::WebResult::Code::LibError, "Room is not registered"};
    }
    using JsonHandle = std::unique_ptr<EmuJson, decltype(&emu_json_free)>;
    JsonHandle json(emu_json_new_object(), &emu_json_free);
    JsonHandle members(emu_json_new_array(), &emu_json_free);
    for (const AnnounceMultiplayerRoom::Room::Member& member : room.members) {
        emu_json_push(members.get(), AnnounceMultiplayerRoom::MemberToJson(member));
    }
    emu_json_set(json.get(), "players", members.release());
    return client.PostJson(fmt::format("/lobby/{}", room_id),
                           AnnounceMultiplayerRoom::Stringify(json.get()), false);
}

Common::WebResult RoomJson::Register() {
    using JsonHandle = std::unique_ptr<EmuJson, decltype(&emu_json_free)>;
    JsonHandle json(AnnounceMultiplayerRoom::RoomToJson(room), &emu_json_free);
    auto result = client.PostJson("/lobby", AnnounceMultiplayerRoom::Stringify(json.get()), false);
    if (result.result_code != Common::WebResult::Code::Success) {
        return result;
    }
    JsonHandle reply_json(emu_json_parse(result.returned_data.data(), result.returned_data.size()),
                          &emu_json_free);
    if (!reply_json || !AnnounceMultiplayerRoom::RoomFromJson(reply_json.get(), room)) {
        return Common::WebResult{Common::WebResult::Code::LibError,
                                 "Failed to parse the room registration reply"};
    }
    std::string id;
    if (!AnnounceMultiplayerRoom::GetString(reply_json.get(), "id", id)) {
        return Common::WebResult{Common::WebResult::Code::LibError,
                                 "The room registration reply has no id"};
    }
    room_id = id;
    return Common::WebResult{Common::WebResult::Code::Success, "", room.verify_UID};
}

void RoomJson::ClearPlayers() {
    room.members.clear();
}

AnnounceMultiplayerRoom::RoomList RoomJson::GetRoomList() {
    auto reply = client.GetJson("/lobby", true).returned_data;
    if (reply.empty()) {
        return {};
    }
    using JsonHandle = std::unique_ptr<EmuJson, decltype(&emu_json_free)>;
    const JsonHandle json(emu_json_parse(reply.data(), reply.size()), &emu_json_free);
    if (!json) {
        LOG_ERROR(WebService, "Failed to parse the room list reply: {}",
                  emu_json_error());
        return {};
    }
    const JsonHandle rooms(emu_json_get(json.get(), "rooms"), &emu_json_free);
    if (!rooms || emu_json_type(rooms.get()) != EMU_JSON_ARRAY) {
        return {};
    }
    AnnounceMultiplayerRoom::RoomList room_list;
    const std::size_t count = emu_json_array_len(rooms.get());
    for (std::size_t i = 0; i < count; ++i) {
        const JsonHandle element(emu_json_array_at(rooms.get(), i), &emu_json_free);
        AnnounceMultiplayerRoom::Room room;
        if (!AnnounceMultiplayerRoom::RoomFromJson(element.get(), room)) {
            LOG_ERROR(WebService, "Failed to parse a room in the room list reply");
            return {};
        }
        room_list.push_back(room);
    }
    return room_list;
}

void RoomJson::Delete() {
    if (room_id.empty()) {
        LOG_ERROR(WebService, "Room must be registered to be deleted");
        return;
    }
    Common::DetachedTasks::AddTask(
        [host{this->host}, username{this->username}, token{this->token}, room_id{this->room_id}]() {
            // create a new client here because the this->client might be destroyed.
            Client{host, username, token}.DeleteJson(fmt::format("/lobby/{}", room_id), "", false);
        });
}

} // namespace WebService
