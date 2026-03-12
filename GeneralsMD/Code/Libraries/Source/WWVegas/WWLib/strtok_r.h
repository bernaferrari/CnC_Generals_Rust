#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __STRTOK_R_H__
#define __STRTOK_R_H__

#ifndef _UNIX
char *strtok_r(char *strptr, const char *delimiters, char **lasts);
#endif

#endif
