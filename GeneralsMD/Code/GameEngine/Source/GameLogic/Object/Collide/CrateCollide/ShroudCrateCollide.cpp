// FILE: ShroudCrateCollide.cpp ///////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2002
// Desc:   A crate that clears the shroud for the pickerupper
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/AudioEventRTS.h"
#include "Common/MiscAudio.h"
#include "Common/Player.h"
#include "Common/Xfer.h"
#include "GameLogic/PartitionManager.h"
#include "GameLogic/Module/ShroudCrateCollide.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ShroudCrateCollide::ShroudCrateCollide( Thing *thing, const ModuleData* moduleData ) : CrateCollide( thing, moduleData )
{

} 

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ShroudCrateCollide::~ShroudCrateCollide( void )
{

}  

//-------------------------------------------------------------------------------------------------
Bool ShroudCrateCollide::executeCrateBehavior( Object *other )
{
	Player* cratePlayer = other->getControllingPlayer();
	ThePartitionManager->revealMapForPlayer( cratePlayer->getPlayerIndex() );

	//Play a crate pickup sound.
	AudioEventRTS soundToPlay = TheAudio->getMiscAudio()->m_crateShroud;
	soundToPlay.setObjectID( other->getID() );
	TheAudio->addAudioEvent(&soundToPlay);

	return TRUE;
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void ShroudCrateCollide::crc( Xfer *xfer )
{

	// extend base class
	CrateCollide::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void ShroudCrateCollide::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	CrateCollide::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void ShroudCrateCollide::loadPostProcess( void )
{

	// extend base class
	CrateCollide::loadPostProcess();

}  // end loadPostProcess
