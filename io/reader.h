/**
 * @file reader.h
 * @author Noah Mingolelli
 * general helpers to read directly from a binary file. files are embedded little endian
 * doccing this later cuz no
 */
#ifndef READER_H
#define READER_H

#include "vm.h"

/**
 * read an exact amt of bits from a packed struct
 */
bool read_exact(FILE* f, void* out, size_t n);

/**
 * read a 16 bit value from a packed struct
 */
u16 read_u16_le(const u8 b[2]);

/**
 * read a 32 bit value from a packed struct
 */
u32 read_u32_le(const u8 b[4]);

/**
 * read and load a file into memory. exclusively deals with the file, then vm_load handles the rest
 */
bool vm_load_file(VM* vm, const char* path);

#endif