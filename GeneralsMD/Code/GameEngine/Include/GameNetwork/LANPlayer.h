// LANPlayer.h ///////////////////////////////////////////////////////////////
// LAN Player Class used for both the LANAPI and LANGameInfo
// Author: Matthew D. Campbell, October 2001 (Pulled out of LANAPI.h by CLH on 12/21/01

#pragma once

#ifndef _LANPLAYER_H_
#define _LANPLAYER_H_


/**
 * LAN player class.  This is for players sitting in the lobby.  Players are
 * uniquely identified by their IP addresses.
 */
class LANPlayer
{
public:
	LANPlayer() { m_name = m_login = m_host = L""; m_lastHeard = 0; m_next = NULL; m_IP = 0; }

	// Access functions
	inline UnicodeString getName( void ) { return m_name; }
	inline void setName( UnicodeString name ) { m_name = name; }
	inline UnicodeString getLogin( void ) { return m_login; }
	inline void setLogin( UnicodeString name ) { m_login = name; }
	inline void setLogin( AsciiString name ) { m_login.translate(name); }
	inline UnicodeString getHost( void ) { return m_host; }
	inline void setHost( UnicodeString name ) { m_host = name; }
	inline void setHost( AsciiString name ) { m_host.translate(name); }
	inline UnsignedInt getLastHeard( void ) { return m_lastHeard; }
	inline void setLastHeard( UnsignedInt lastHeard ) { m_lastHeard = lastHeard; }
	inline LANPlayer *getNext( void ) { return m_next; }
	inline void setNext( LANPlayer *next ) { m_next = next; }
	inline UnsignedInt getIP( void ) { return m_IP; }
	inline void setIP( UnsignedInt IP ) { m_IP = IP; }
	
protected:
	UnicodeString m_name;			///< Player name
	UnicodeString m_login;		///< login name
	UnicodeString m_host;			///< machine name
	UnsignedInt m_lastHeard;	///< The last time we heard from this player (for timeout purposes)
	LANPlayer *m_next;				///< Linked list pointer
	UnsignedInt m_IP;					///< Player's IP
};

#endif //_LANPLAYER_H_