/******************************************************************************
*
* FILE
*     $Archive:  $
*
* DESCRIPTION
*     Perform ANSI <-> Unicode string conversions
*
* PROGRAMMER
*     Denzil E. Long, Jr.
*     $Author:  $
*
* VERSION INFO
*     $Modtime:  $
*     $Revision:  $
*
******************************************************************************/

#include "StringConvert.h"
#include "UString.h"
#include <windows.h>
#include <Debug\DebugPrint.h>
#include <assert.h>

/******************************************************************************
*
* NAME
*     UStringToANSI
*
* DESCRIPTION
*     Convert UString to an ANSI string
*
* INPUTS
*     String - String to convert
*     Buffer - Pointer to buffer to receive conversion.
*     BufferLength - Length of buffer
*
* RESULT
*     ANSI - Pointer to ANSI string
*
******************************************************************************/

Char* UStringToANSI(const UString& string, Char* buffer, UInt bufferLength)
	{
	return UnicodeToANSI(string.Get(), buffer, bufferLength);
	}


/******************************************************************************
*
* NAME
*     UnicodeToANSI
*
* DESCRIPTION
*     Convert Unicode string to an ANSI string
*
* INPUTS
*     String - Unicode string to convert
*     Buffer - Pointer to buffer to receive conversion.
*     BufferLength - Length of buffer
*
* RESULT
*     ANSI - Pointer to ANSI string
*
******************************************************************************/

Char* UnicodeToANSI(const WChar* string, Char* buffer, UInt bufferLength)
	{
	if ((string == NULL) || (buffer == NULL))
		{
		return NULL;
		}

	#ifdef _DEBUG
	int result = 
	#endif
		WideCharToMultiByte(CP_ACP, 0, string, -1, buffer, bufferLength,
			NULL, NULL);

	#ifdef _DEBUG
	if (result == 0)
		{
		PrintWin32Error("ConvertToANSI() Failed");
		assert(false);
		}
	#endif

	return buffer;
	}
