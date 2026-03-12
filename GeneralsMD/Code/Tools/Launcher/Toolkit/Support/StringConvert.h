/******************************************************************************
*
* FILE
*     $Archive:  $
*
* DESCRIPTION
*     ANSI <-> Unicode string conversions
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

#ifndef STRINGCONVERT_H
#define STRINGCONVERT_H

#include "UTypes.h"

class UString;

Char* UStringToANSI(const UString& string, Char* buffer, UInt bufferLength);
Char* UnicodeToANSI(const WChar* string, Char* buffer, UInt bufferLength);

#endif // STRINGCONVERT_H
