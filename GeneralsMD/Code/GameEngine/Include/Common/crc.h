// CRC.h ///////////////////////////////////////////////////////////////
// A class encapsulating CRC calculation
// Author: Matthew D. Campbell, October 2001

#pragma once

#ifndef _CRC_H_
#define _CRC_H_

#include "Lib/BaseType.h"
#include "winsock2.h" // for htonl

#ifdef _DEBUG

class CRC
{
public:
	CRC() { crc = 0; }

	void computeCRC( const void *buf, Int len );		///< Compute the CRC for a buffer, added into current CRC
	void clear( void ) { crc = 0; }									///< Clears the CRC to 0
//	UnsignedInt get( void ) { return htonl(crc); }	///< Get the combined CRC
	UnsignedInt get( void );

private:
	void addCRC( UnsignedByte val );									///< CRC a 4-byte block

	UnsignedInt crc;
};

#else

// optimized inline only version
class CRC
{
public:
	CRC(void) { crc=0; }

  /// Compute the CRC for a buffer, added into current CRC
	__forceinline void computeCRC( const void *buf, Int len )
  {
    if (!buf||len<1)
      return;
    
    /* C++ version left in for reference purposes
	  for (UnsignedByte *uintPtr=(UnsignedByte *)buf;len>0;len--,uintPtr++)
    {
    	int hibit;
    	if (crc & 0x80000000) 
      {
		    hibit = 1;
	    } 
      else 
      {
		    hibit = 0;
	    }

	    crc <<= 1;
	    crc += *uintPtr;
	    crc += hibit;
    }
    */

    // ASM version, verified by comparing resulting data with C++ version data
    unsigned *crcPtr=&crc;
    _asm
    {
      mov esi,[buf]
      mov ecx,[len]
      dec ecx
      mov edi,[crcPtr]
      mov ebx,dword ptr [edi]
      xor eax,eax
    lp:
      mov al,byte ptr [esi]
      shl ebx,1
      inc esi
      adc ebx,eax
      dec ecx
      jns lp
      mov dword ptr [edi],ebx
    };
  }

  /// Clears the CRC to 0
	void clear( void ) 
  { 
    crc = 0; 
  }									

  ///< Get the combined CRC
	UnsignedInt get( void ) const
  {
    return crc;
  }

private:
	UnsignedInt crc;
};

#endif

#endif // _CRC_H_
