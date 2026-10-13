#ifndef EMU_MEM_H
#define EMU_MEM_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/** The physical memory of the emulated 3DS, which the crate allocates and owns. */
typedef struct EmuMem EmuMem;

/** The page table of a process, which the crate allocates and owns. */
typedef struct EmuMemPageTable EmuMemPageTable;

/** The values of `Memory::Region`. The DSP memory is owned by the DSP, so it has no pointer here. */
enum {
  EMU_MEM_REGION_FCRAM = 0,
  EMU_MEM_REGION_VRAM = 1,
  EMU_MEM_REGION_DSP = 2,
  EMU_MEM_REGION_N3DS = 3,
};

/** The values of `Memory::PageType`. */
enum {
  EMU_MEM_PAGE_UNMAPPED = 0,
  EMU_MEM_PAGE_MEMORY = 1,
  EMU_MEM_PAGE_RASTERIZER_CACHED_MEMORY = 2,
};

/**
 * The arrays of a page table, with one entry per page of 4 KiB. They stay valid until the page
 * table is destroyed. The core reads them directly and writes them through the functions below.
 */
typedef struct EmuMemPageTableView {
  uint8_t** entries;
  uint8_t* attributes;
} EmuMemPageTableView;

EmuMem* emu_mem_create(void);
void emu_mem_destroy(EmuMem* memory);
uint8_t* emu_mem_region_ptr(const EmuMem* memory, uint32_t region);
size_t emu_mem_region_size(uint32_t region);

EmuMemPageTable* emu_mem_pt_create(void);
void emu_mem_pt_destroy(EmuMemPageTable* page_table);
EmuMemPageTableView emu_mem_pt_view(const EmuMemPageTable* page_table);
void emu_mem_pt_clear(const EmuMemPageTable* page_table);
void emu_mem_pt_set_entry(const EmuMemPageTable* page_table, uint32_t page, uint8_t* pointer);
void emu_mem_pt_set_attribute(const EmuMemPageTable* page_table, uint32_t page, uint8_t type);

#ifdef __cplusplus
}
#endif

#endif
