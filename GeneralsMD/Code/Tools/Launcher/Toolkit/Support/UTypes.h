/******************************************************************************
*
* FILE
*     $Archive:  $
*
* DESCRIPTION
*     Generic user type definitions
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

#ifndef UTYPES_H
#define UTYPES_H

//! Signed integer value
typedef int Int;

//! Unsigned integer value
typedef unsigned int UInt;

//! Signed 8bit value (-127 - 128)
typedef char Int8;

//! Unsigned 8bit value (0 - 255)
typedef unsigned char UInt8;

//! Signed 16bit value (-32767 - 32768)
typedef short Int16;

//! Unsigned 16bit value (0 - 65535)
typedef unsigned short UInt16;

//! Signed 32bit value
typedef long Int32;

//! Unsigned 32bit value
typedef unsigned long UInt32;

//! Signed character (ASCII)
typedef char Char;

//! Unsigned character (ANSI)
typedef unsigned char UChar;

//! Wide character (Unicode)
typedef unsigned short WChar;

//! 32bit floating point value
typedef float Float32;

//! 64bit floating point value
typedef double Float64;

//! Floating point value
typedef Float32 Float;

//! TriState
typedef enum {OFF = false, ON = true, PENDING = -1} TriState;

//! Empty pointer
#ifndef NULL
#define NULL (0L)
#endif

#endif // UTYPES_H
