// FILE: MissionStats.h ////////////////////////////////////////////////////////////
//
// Project:    RTS3
//
// File name:  MissionStats.h
//
// Created:    Steven Johnson, October 2001
//
// Desc:			 @todo
//
//-----------------------------------------------------------------------------

#pragma once

#ifndef _MISSIONSTATS_H_
#define _MISSIONSTATS_H_

#include "Lib/BaseType.h"
#include "Common/GameCommon.h"
#include "Common/Snapshot.h"

// ----------------------------------------------------------------------------------------------
/**
	Class that accumulates stats during a mission. Some of this will be for scoring purposes, 
	and some will probably be used by AI to determine future moves.

	@todo: not sure what need to be here. Alas. For now, I have just put in the fields from RA2
	that are indicated as being used for scoring in multiplayer games, so this will certainly
	increase.
*/
class MissionStats : public Snapshot
{

public:
	
	MissionStats();

	/// reset all stats to "nothing".
	void init();

protected:

	// snapshot methods
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

private:

	Int m_unitsKilled[MAX_PLAYER_COUNT];					///< how many units for each Player were killed by us?
	Int m_unitsLost;															///< how many of our units were destroyed?
	Int m_buildingsKilled[MAX_PLAYER_COUNT];			///< how many buildings for each Player were killed by us?
	Int m_buildingsLost;													///< how many of our buildings were destroyed?
	//Int	m_whoLastHurtMe;													///< last Player to destroy one of my units
};

#endif // _MISSIONSTATS_H_

