#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef DATA_H
#define DATA_H

#include	"buff.h"
#include	"iff.h"
#include	"win.h"
#include	"wwfile.h"

#define TXT_NONE	0

int Load_Picture(FileClass &file, Buffer & scratchbuf, Buffer & destbuf, unsigned char * palette, PicturePlaneType format);
void * Load_Alloc_Data(FileClass & file);
long Load_Uncompress(FileClass & file, Buffer & uncomp_buff, Buffer & dest_buff, void * reserved_data);
char const * Fetch_String(int id);
void const * Fetch_Resource(LPCSTR resname, LPCSTR restype);

#endif
