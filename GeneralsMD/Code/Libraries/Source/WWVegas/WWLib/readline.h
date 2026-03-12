#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef READLINE_H
#define READLINE_H

#include	"straw.h"
#include	"wwfile.h"
#include <wchar.h>

int Read_Line(FileClass & file, char * buffer, int len, bool & eof);
int Read_Line(Straw & file, char * buffer, int len, bool & eof);
int Read_Line(Straw & file, wchar_t * buffer, int len, bool & eof);

#endif
