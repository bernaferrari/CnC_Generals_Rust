#if _MSC_VER >= 1000
#pragma once
#endif // _MSC_VER >= 1000

#ifndef REALCRC_H
#define REALCRC_H


unsigned long	CRC_Memory( const unsigned char *data, unsigned long length, unsigned long crc = 0 );
unsigned long	CRC_String( const char *string, unsigned long crc = 0 );
unsigned long	CRC_Stringi( const char *string, unsigned long crc = 0 );


#endif
