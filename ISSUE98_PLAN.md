# Issue #98 実装プラン: 標準 JSON DOM (serde_json)

- 対象: [CamelEmu/CamelEmu#98](https://github.com/CamelEmu/CamelEmu/issues/98)「標準 JSON DOM (serde_json)」
- 親トラッカー: #100「core externals の Rust 実装への移行トラッカー」
- 前提 Issue: #94(Phase 0: `packages/camel_emu` 骨格と session への移動)
- 後続 Issue: #99(`packages/core/externals` の除去。本 Issue では json の除去自体は行わない)
- 調査時点の HEAD: `0648347d3` (main, Merge PR #93)

---

## 1. ゴールと境界

標準 JSON(RFC 8259)DOM だけを担当する Rust クレート `json` を新設し、C ABI `emu_json_*`
(`packages/native/include/emu_json.h`)として公開する。

境界に持ち込まないもの:

- nlohmann 独自型: `ordered_json`、`nlohmann::detail::out_of_range` 等の例外型
- 独自仕様の写像(Room の `to_json`/`from_json`、欠落フィールドの扱い)は C++ 側 shim に隔離する

挿入順保持は ABI 規約として明記し、`ordered_json` 相当の出力安定性をこれで担保する。

## 2. 現状調査(実測)

### 2.1 リポジトリ状態

- `packages/camel_emu` は**まだ存在しない**。現行構成は `packages/{azahar_rust, core, externals, native}`。
  → 本 Issue は #94(workspace 化、members: `session`, `crypto`, `random`, `http`, `json`)完了が前提。
  #94 未マージで先行する場合は、json クレート分だけ最小スキャフォールドを作り、#94 のパス/命名
  (`packages/camel_emu/externals/json`)に揃えて競合を避ける。
- 既存クレート `azahar_rust`(→ `session`)の型を踏襲する:
  `crate-type = ["cdylib", "staticlib", "lib"]`、edition 2024、unsafe は `ffi` モジュールに局所化、
  ヘッダは opaque ポインタ + `extern "C"` + 関数ごとの所有権コメントというスタイル。

### 2.2 nlohmann 利用箇所(Issue 記載どおり 3 TU のみ)

| ファイル | 利用内容 |
|---|---|
| `packages/core/src/web_service/verify_login.cpp` | `json::parse(reply)` → `find("username")` → 文字列比較のみ |
| `packages/core/src/web_service/announce_room_json.{h,cpp}` | `to_json`/`from_json`(Room/Member)、`out_of_range` catch が 3 箇所、`.at().get<T>()`(u16/u32/u64/bool/string)、`parse`、`dump()`、`{"players", members}` 構築 |
| `packages/core/src/video_core/custom_textures/custom_tex_manager.cpp` | `ordered_json` で pack.json 生成し `dump(4)`、`parse(config, nullptr, false, true)`(例外なし・**コメント許可**)、`options` の `get<bool>()`、`textures.items()` のキー順反復、`is_string()/is_array()/get<vector<string>>()` |

CMake の現行結線: `video_core` と `web_service` が `json-headers` をリンク
(`packages/core/src/video_core/CMakeLists.txt:211`、`packages/core/src/web_service/CMakeLists.txt:16`)。

## 3. Step 0: ABI の確定(Issue 記載 ABI の不足を先に潰す)

Issue 記載の ABI は骨子のみで、そのままでは接続先 3 ファイルを実装し切れない。
着手前に以下を確定し、`emu_json.h` に所有権・寿命ごと明記する。

| # | 論点 | 現行コードの要求 | 提案(推奨) |
|---|---|---|---|
| 1 | 数値の生成・取得型 | u16/u32/u64(`gameId` 等)、bool の書き込みが必須。Issue の生成系に `new_number`/`new_bool`/`new_null` が無い | `emu_json_new_number_i64/u64/f64`、`emu_json_new_bool`、`emu_json_new_null` を追加。取得も `emu_json_i64/u64/f64` に分離し、`emu_json_number` は f64 として残すか削除するか決める(u64 を f64 経由にすると 2^53 超で精度落ちするため不可) |
| 2 | 型判定 | `ReadConfig` が `is_string()/is_array()` を使用 | `emu_json_type(handle) -> enum(Null/Bool/Number/String/Array/Object)` を追加 |
| 3 | オブジェクト反復 | `textures.items()` でキーと値を挿入順に走査 | `emu_json_object_len(handle)` + `emu_json_key_at(handle, index) -> const char*` を追加(順序=挿入順を規約化) |
| 4 | 整形出力 | pack.json は `dump(4)`(インデント 4) | `emu_json_stringify_indent(handle, indent, len)` を追加するか、`stringify` に indent 引数を持たせる |
| 5 | `emu_json_get` の path 形式と戻り値の所有権 | ネストした取得(`rooms` 配下、配列要素)が必要 | path 文法を仕様化(ドット区切りキー + `[index]`、キーのエスケープ規則)。戻り値は**借用ハンドル**(親が所有、子を `emu_json_free` しない、親の変更・解放で無効)を推奨し明記する |
| 6 | `emu_json_set`/`push` の所有権 | 値オブジェクトを組み立てて親へ渡す | value は consume(所有権移転)と明記。失敗時に所有権がどうなるかも定義する |
| 7 | `emu_json_stringify` 結果の解放 | Issue に解放関数が無い | `emu_json_free_string(char*)` を追加。`emu_json_string` は借用(`const char*`、親の変更・解放まで有効)として分離する |
| 8 | `emu_json_error` の仕様 | parse 失敗理由の取得 | thread_local な最終エラー文字列を返す。次の `emu_json_*` 呼び出しで上書きされる旨を明記 |
| 9 | parse のコメント扱い | `ReadConfig` は `ignore_comments=true`。serde_json はコメント非対応 | 差分を仕様として受容し、失敗時は「parse 失敗 → デフォルト動作(ReadConfig は false を返す)」に倒すか、C++ shim 側でコメント除去するか決める。**推奨: 受容**(pack.json は本エミュレータが生成する形式でコメントは必須要件でない)。判断は PR 説明に残す |
| 10 | parse 失敗時の既存挙動 | `verify_login` と `announce_room_json` は例外送出型 parse を使用(失敗時は例外が伝播) | ABI は NULL + `emu_json_error` に統一。呼び出し側では「失敗 = 空応答と同等に扱い false/デフォルトを返す + ログ」を推奨し、現行との挙動差として明記する |

## 4. Step 1: クレート実装 `packages/camel_emu/externals/json`

- `Cargo.toml`: workspace member `json` として追加(session クレートの様式に合わせる)。
  依存は `serde_json = { version = "1", features = ["preserve_order"] }` と `libc` のみ。
  `preserve_order`(= IndexMap バックエンド)が挿入順保持の根拠。Why コメントはここにだけ付ける。
- `src/lib.rs`: `Box<serde_json::Value>` を所有する opaque 構造体 `EmuJson` と FFI 群。
  unsafe は FFI 境界モジュールに局所化し(session クレートの `lib.rs` 方針を踏襲)、
  全関数で NULL チェック + `catch_unwind` で panic を境界の外に出さない。
- 実装メモ:
  - `emu_json_parse`: `serde_json::from_slice`。失敗時はエラー文字列を thread_local に格納して NULL。
  - `emu_json_get`: path を解釈して借用ハンドルを返すため、内部的には子 Value への
    ポインタを包む非所有ハンドル型にする(所有ハンドルと型を分けるか、フラグで区別)。
    ここが本クレート最大の設計ポイント。
  - `emu_json_stringify`: `to_string` / `to_string_pretty`(indent 指定時は空白数の扱いを
    nlohmann の `dump(4)` と一致させる)。
  - 数値: serde_json 標準の Number(i64/u64/f64)で保持。`arbitrary_precision` は有効化しない。
- ドキュメントコメントは英語、Why が必要な箇所のみ(挿入順保持の理由など)。Issue のコーディングルール準拠。

## 5. Step 2: Phase 2 再配線(C++ 側)

### 5.1 `verify_login.cpp`(parse のみ)

- `#include <json.hpp>` → `#include "emu_json.h"`。
- `parse` → `emu_json_parse`、NULL なら失敗として `false`(Step 0-10 の決定に従う)。
- `find("username")` → `emu_json_get(json, "username")`、NULL なら現行どおり `username.empty()` を返す。
  一致判定は `emu_json_string` との文字列比較。解放順序(借用ハンドルを先に使い切ってから `emu_json_free`)に注意。

### 5.2 `announce_room_json.{h,cpp}`(写像は C++ 残置)

- `to_json`/`from_json` のフィールド写像と欠落時の既定値処理は C++ shim のまま、
  実装基盤だけを `emu_json_*` に置換する。挿入順は現行の代入順を厳密に維持する:
  - Member: `username`(空でない場合のみ) → `nickname` → `avatarUrl`(同上) → `gameName` → `gameId`
  - Room: `port` → `name` → `description`(同上) → `preferredGameName` → `preferredGameId` →
    `maxPlayers` → `netVersion` → `hasPassword` → `players`(メンバーがいる場合のみ)
- `out_of_range` catch 3 箇所(Member の username/avatarUrl、Room の description、
  Room の players)は、`emu_json_get` の NULL 判定 + 型判定への置換で「status 化」する(例外型は境界に残さない)。
  ログ出力(LOG_DEBUG)は現行どおり残す。
- 必須フィールド(`.at()` 相当)は NULL/型不一致時に現行の例外伝播と等価な失敗として扱う方針を決め、
  呼び出し側(`Update`/`GetRoomList`)の `WebResult` への反映を確認する。
- `Update` の `{{"players", room.members}}` 構築と `dump()`、`GetRoomList` の
  `parse(...).at("rooms")` + 配列走査(`emu_json_array_len` + 添字 path)を置換する。

### 5.3 `custom_tex_manager.cpp`(ordered_json → emu_json)

- `PrepareDumping`: `author` → `version` → `description` → `options{skip_mipmap, flip_png_files, use_new_hash}`
  の順で構築し、整形 stringify(インデント 4)で書き出す。出力バイト列が現行と一致することを回帰観点にする。
- `ReadConfig`: parse 失敗時は現行(例外なし parse + 後段の値取得で失敗)と同様に `false` を返す流れに統一。
  `options` の bool 3 件取得、`textures` オブジェクトのキー順反復(Step 0-3 の API)、
  値の string/array 判定と文字列配列の取り出しを置換する。
- `#include <json.hpp>` を削除。

### 5.4 CMake 結線

- `video_core` / `web_service` の `json-headers` リンクを emu_json の imported static lib に置換する
  範囲は #99 と重なるため、#99 の CMake 作業項目(web_service / video_core の更新)と担当範囲を事前に切り分ける。
  本 Issue の検証(最終リンク)が通る最小限の結線のみを本 Issue で行い、externals 本体の削除は #99 に残す。

## 6. Step 3: テスト・検証(Issue の「検証」に対応)

### クレート単体(`cargo test`、`packages/camel_emu`)

- **挿入順保持**: キーを `b, a, c` の順で追加 → stringify 順が同順であること(Issue 指定テスト)
- parse/stringify ラウンドトリップ、ネストした path 取得、存在しないキー → NULL
- u64 最大値を含む数値の精度、文字列のエスケープ/Unicode
- parse 失敗時に `emu_json_error` で理由が取れること
- 配列の `push`/`array_len`、オブジェクトのキー反復順

### C++ 側リグレッション

- custom_tex_manager: pack.json の読み書き(既存 pack.json の読込、生成 pack.json の内容・順序・インデント比較)
- announce_room_json: Member/Room の to/from ラウンドトリップ(任意フィールド欠落ケース含む)
- verify_login: username 一致/不一致/欠落/空 の各ケース

### ビルド・リンク

- `cargo build`(`packages/camel_emu`)
- `flutter build linux --debug` / `flutter build apk --debug` で `emu_json_*` シンボルが解決すること(Issue 指定)
- AGENTS.md の手順どおり `.env.sh` を source し、ビルドログは全文確認する

## 7. 作業順序と規模感(目安)

| Step | 内容 | 規模感 |
|---|---|---|
| 0 | ABI 確定(Step 0 の表、特に 1/3/5/9/10) | 設計のみ。ここが遅れると全工程が手戻りする |
| 1 | json クレート + `emu_json.h` + 単体テスト | Rust 約 400〜600 行 + ヘッダ約 100 行 + テスト約 200 行 |
| 2 | 再配線 3 ファイル + CMake 最小結線 | C++ 差分 約 200〜300 行 |
| 3 | リグレッション + Linux/Android リンク確認 | テストケース追加とビルド確認 |

前提: #94 のマージ。未マージの場合は Step 1 の前にスキャフォールド調整が入る。

## 8. リスク・注意点

- **数値フォーマット差**: serde_json と nlohmann で浮動小数点の文字列表現が異なる場合がある
  (対象 3 ファイルでは float を直接出力しないため影響は限定的だが、stringify の回帰で確認する)。
- **重複キー**: parse 時の重複キーの扱い(後勝ち/順序位置)が両者で微妙に異なる。pack.json・API 応答では
  実害は想定しにくいが、仕様差として記録する。
- **`preserve_order` の feature 統合**: workspace 内の他クレートが serde_json に依存すると
  feature が統合される。順序保持が workspace 全体で有効になる前提で問題ないか確認する。
- **借用ハンドルの寿命**: `emu_json_get` の戻り値を親の変更・解放後に使うと UAF になる。
  ヘッダに寿命を明記し、C++ 側の利用を「取得→即利用→解放」の形に統一する。
- **whole-archive リンク**: session クレートと同様、staticlib のシンボルが最終リンクで
  落ちないよう、リンク側の扱い(#99 の結線作業と連携)を確認する。

## 9. スコープ外

- `packages/core/externals/json` の削除と CMake からの `json-headers` 完全除去(#99)
- 他クレート(crypto #95 / random #96 / http #97)の実装
- `session` の分離(#101/#102)、統合クレートへの集約(#103)

## 10. 完了条件(Issue の検証項目への対応)

- [ ] Linux / Android の最終リンクで `emu_json_*` シンボルが解決する
- [ ] 挿入順保持のユニットテスト(キー追加順 = stringify 順)が通る
- [ ] custom_tex_manager の info.json(pack.json)読み書きリグレッションが通る
- [ ] 対象 3 ファイルから `#include <json.hpp>` と nlohmann 型が消えている
- [ ] `emu_json.h` に ABI 規約(挿入順保持、所有権、寿命、path 文法、エラー取得)が明記されている
