#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/CRC.h"
#include "Common/Debug.h"

#ifdef _DEBUG

void CRC::addCRC( UnsignedByte val )
{
	int hibit;

	//cout << "\t\t" << hex << val;
//	val = htonl(val);
	//cout << " / " << hex << val <<endl;


	if (crc & 0x80000000) {
		hibit = 1;
	} else {
		hibit = 0;
	}

	crc <<= 1;
	crc += val;
	crc += hibit;

	//cout << hex << (*crc) <<endl;
}


void CRC::computeCRC( const void *buf, Int len )
{
	if (!buf || len < 1)
	{
		return;
	}

	//crc = 0;

	UnsignedByte *uintPtr = (UnsignedByte *)buf;

	for (int i=0 ; i<len ; i++) {
		addCRC (*(uintPtr++));
	}
	//crc = htonl(crc);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UnsignedInt CRC::get( void )
{

	UnsignedInt tcrc = crc;
	return tcrc;

}  // end skip

#endif
