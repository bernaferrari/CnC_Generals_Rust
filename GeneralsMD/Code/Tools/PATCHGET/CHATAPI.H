#ifndef CHATAPI_HEADER
#define CHATAPI_HEADER

#include "cominit.h"
#include <stdio.h>
#include <windows.h>
#include <commctrl.h>
#include <winerror.h>
#include <ocidl.h>
#include <olectl.h>

/**********************************************************************
**	This macro serves as a general way to determine the number of elements
**	within an array.
*/
#define	ARRAY_SIZE(x)		int(sizeof(x)/sizeof(x[0]))
#define size_of(typ,id) sizeof(((typ*)0)->id)


void Startup_Chat(void);
void Shutdown_Chat(void);
void Update_If_Required(void);

char const * Fetch_String(int id);

#endif
