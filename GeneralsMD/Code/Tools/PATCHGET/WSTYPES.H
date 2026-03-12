/****************************************************************************\
*        C O N F I D E N T I A L --- W E S T W O O D   S T U D I O S         *
******************************************************************************
Project Name:
File Name   : wstypes.h
Author      : Neal Kettler
Start Date  : June 3, 1997
Last Update : June 17, 1997  

Standard type definitions for the sake of portability and readability.
\***************************************************************************/

#ifndef WTYPES_HEADER
#define WTYPES_HEADER

#ifndef TRUE
#define TRUE 1
#endif

#ifndef FALSE
#define FALSE 0
#endif


//These are used for readability purposes mostly, when a method takes a
//  pointer or reference these help specify what will happen to the data
//  that is sent in.
#define IN
#define OUT
#define INOUT

typedef char               bit8;


typedef char               sint8;
typedef unsigned char      uint8;
typedef signed short int   sint16;
typedef unsigned short int uint16;
typedef signed int         sint32;
typedef unsigned int       uint32;


#define MAX_BIT8    0x1
#define MAX_UINT32  0xFFFFFFFF
#define MAX_UINT16  0xFFFF
#define MAX_UINT8   0xFF
#define MAX_SINT32  0x7FFFFFFF
#define MAX_SINT16  0x7FFF
#define MAX_SINT8   0x7F   

#ifdef _WIN32
#define strncasecmp _strnicmp
#define strcasecmp  _stricmp
#endif  

#endif
