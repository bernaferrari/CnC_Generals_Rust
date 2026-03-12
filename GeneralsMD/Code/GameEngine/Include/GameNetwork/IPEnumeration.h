// IPEnumeration.h ///////////////////////////////////////////////////////////////
// Class for enumerating IP addresses
// Author: Matthew D. Campbell, October 2001

#pragma once

#ifndef _IPENUMERATION_H_
#define _IPENUMERATION_H_

#include "GameNetwork/Transport.h"

/**
 * IP wrapper class.
 */
class EnumeratedIP : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(EnumeratedIP, "EnumeratedIP")		
public:
	EnumeratedIP() { m_IPstring = ""; m_next = NULL; m_IP = 0; }

	// Access functions
	inline AsciiString getIPstring( void ) { return m_IPstring; }
	inline void setIPstring( AsciiString name ) { m_IPstring = name; }
	inline UnsignedInt getIP( void ) { return m_IP; }
	inline void setIP( UnsignedInt IP ) { m_IP = IP; }
	inline EnumeratedIP *getNext( void ) { return m_next; }
	inline void setNext( EnumeratedIP *next ) { m_next = next; }

protected:
	AsciiString m_IPstring;
	UnsignedInt m_IP;
	EnumeratedIP *m_next;
};
EMPTY_DTOR(EnumeratedIP)


/**
 * The IPEnumeration class is used to obtain a list of IP addresses on the
 * local machine.
 */
class IPEnumeration
{
public:

	IPEnumeration();
	~IPEnumeration();

	EnumeratedIP * getAddresses( void );		///< Return a linked list of local IP addresses
	AsciiString getMachineName( void );			///< Return the Network Neighborhood machine name

protected:

	EnumeratedIP *m_IPlist;
	Bool m_isWinsockInitialized;
};


#endif // _IPENUMERATION_H_
