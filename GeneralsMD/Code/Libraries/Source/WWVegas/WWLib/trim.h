#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef TRIM_H
#define TRIM_H

#include <wchar.h>

char * strtrim(char * buffer);
wchar_t * wcstrim(wchar_t * buffer);

#endif
