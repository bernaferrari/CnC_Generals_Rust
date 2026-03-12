/**
 * User class used by the network.
 */

#pragma once

#ifndef __USER_H
#define __USER_H

#include "GameNetwork/networkdefs.h"
#include "Common/UnicodeString.h"

class User : public MemoryPoolObject
{
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(User, "User")		
public:
	User() {}
	User(UnicodeString name, UnsignedInt addr, UnsignedInt port);
	User &operator= (const User *other);
	Bool operator== (const User *other);
	Bool operator!= (const User *other);

	inline UnicodeString GetName() { return m_name; }
	void setName(UnicodeString name);
	inline UnsignedShort GetPort() { return m_port; }
	inline UnsignedInt GetIPAddr() { return m_ipaddr; }
	inline void SetPort(UnsignedShort port) { m_port = port; }
	inline void SetIPAddr(UnsignedInt ipaddr) { m_ipaddr = ipaddr; }


private:
	UnicodeString m_name;
	UnsignedShort m_port;
	UnsignedInt m_ipaddr;
};
EMPTY_DTOR(User)

#endif
