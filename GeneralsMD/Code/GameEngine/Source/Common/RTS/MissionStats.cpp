// FILE: MissionStats.cpp /////////////////////////////////////////////////////////
//
// Project:   RTS3
//
// File name: MissionStats.cpp
//
// Created:   Steven Johnson, October 2001
//
// Desc:      @todo
//
//-----------------------------------------------------------------------------

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/MissionStats.h"
#include "Common/Player.h"
#include "Common/Xfer.h"

//-----------------------------------------------------------------------------
MissionStats::MissionStats() 
{
	init();
}

//-----------------------------------------------------------------------------
void MissionStats::init() 
{
	Int i;

	for (i = 0; i < MAX_PLAYER_COUNT; ++i)
	{
		m_unitsKilled[i] = 0;
		m_buildingsKilled[i] = 0;
	}
	m_unitsLost = 0;
	m_buildingsLost = 0;
	//m_whoLastHurtMe = PLAYER_NONE;
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void MissionStats::crc( Xfer *xfer )
{

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info;
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void MissionStats::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// units killed
	xfer->xferUser( m_unitsKilled, sizeof( Int ) * MAX_PLAYER_COUNT );

	// units lost
	xfer->xferInt( &m_unitsLost );

	// buidings killed
	xfer->xferUser( m_buildingsKilled, sizeof( Int ) * MAX_PLAYER_COUNT );

	// buildings lost
	xfer->xferInt( &m_buildingsLost );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void MissionStats::loadPostProcess( void )
{

}  // end loadPostProcess
