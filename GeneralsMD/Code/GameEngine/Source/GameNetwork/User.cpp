////////////////////////////////////////////////////////////////////////////////
// User class copy and comparisons

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "GameNetwork/User.h"

/**
 * Constructor.  Sets up the member variables with the appropriate values.
 */
User::User(UnicodeString name, UnsignedInt addr, UnsignedInt port) {
	m_name.set(name);
	m_ipaddr = addr;
	m_port = port;
}

/**
 * The assignment operator.
 */
User & User::operator= (const User *other)
{
	m_name = other->m_name;
	m_ipaddr = other->m_ipaddr;
	m_port = other->m_port;

	return *this;
}

/**
 * The equality operator.
 */
Bool User::operator== (const User *other)
{
	return (m_name.compare(other->m_name) == 0);
}

/**
 * The inequality operator.
 */
Bool User::operator!= (const User *other)
{
	return (m_name.compare(other->m_name) != 0);
}

/**
 * Set the name of this user.
 */
void User::setName(UnicodeString name) {
	m_name.set(name);
}