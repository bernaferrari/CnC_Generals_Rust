///////////////////////////////////////////////////////////////////////////////////////
// FILE: QuickmatchPreferences.h
// Author: Matthew D. Campbell, August 2002
// Description: Saving/Loading of quickmatch preferences
///////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __QUICKMATCHPREFERENCES_H__
#define __QUICKMATCHPREFERENCES_H__

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "Common/UserPreferences.h"

//-----------------------------------------------------------------------------
// QuickMatchPreferences base class 
//-----------------------------------------------------------------------------
class QuickMatchPreferences : public UserPreferences
{
public:
	QuickMatchPreferences();
	virtual ~QuickMatchPreferences();

	void setMapSelected(const AsciiString& mapName, Bool selected);
	Bool isMapSelected(const AsciiString& mapName);

	void setLastLadder(const AsciiString& addr, UnsignedShort port);
	AsciiString getLastLadderAddr( void );
	UnsignedShort getLastLadderPort( void );

	void setMaxDisconnects(Int val);
	Int getMaxDisconnects( void );

	void setMaxPoints(Int val);
	Int getMaxPoints( void );

	void setMinPoints(Int val);
	Int getMinPoints( void );

	void setWaitTime(Int val);
	Int getWaitTime( void );

	void setNumPlayers(Int val);
	Int getNumPlayers( void );

	void setMaxPing(Int val);
	Int getMaxPing( void );

	void setColor(Int val);
	Int getColor( void );

	void setSide(Int val);
	Int getSide( void );
};

#endif // __QUICKMATCHPREFERENCES_H__
