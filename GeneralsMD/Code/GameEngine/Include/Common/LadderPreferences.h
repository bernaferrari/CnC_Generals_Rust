///////////////////////////////////////////////////////////////////////////////////////
// FILE: LadderPreferences.h
// Author: Matthew D. Campbell, August 2002
// Description: Saving/Loading of ladder preferences
///////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __LADDERPREFERENCES_H__
#define __LADDERPREFERENCES_H__

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "Common/UserPreferences.h"

//-----------------------------------------------------------------------------
// LadderPreferences base class 
//-----------------------------------------------------------------------------

class LadderPref
{
public:
	UnicodeString name;
	AsciiString address;
	UnsignedShort port;
	time_t lastPlayDate;

	bool operator== (const LadderPref& other)
	{
		return ( address==other.address && port==other.port );
	}
};

typedef std::map<time_t, LadderPref> LadderPrefMap;

//-----------------------------------------------------------------------------
// LadderPreferences base class 
//-----------------------------------------------------------------------------
class LadderPreferences : public UserPreferences
{
public:
	LadderPreferences();
	virtual ~LadderPreferences();

	Bool loadProfile( Int profileID );
	virtual bool write( void );

	const LadderPrefMap& getRecentLadders( void );
	void addRecentLadder( LadderPref ladder );

private:
	LadderPrefMap m_ladders;
};

#endif // __LADDERPREFERENCES_H__
