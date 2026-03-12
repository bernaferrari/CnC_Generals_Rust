// FILE: MoneyCrateCollide.cpp ///////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2002
// Desc:   A crate that heals everything owned by the collider
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/AudioEventRTS.h"
#include "Common/MiscAudio.h"
#include "Common/Player.h"
#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/HealCrateCollide.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
HealCrateCollide::HealCrateCollide( Thing *thing, const ModuleData* moduleData ) : CrateCollide( thing, moduleData )
{

} 

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
HealCrateCollide::~HealCrateCollide( void )
{

}  

//-------------------------------------------------------------------------------------------------
Bool HealCrateCollide::executeCrateBehavior( Object *other )
{
	Player* cratePlayer = other->getControllingPlayer();
	cratePlayer->healAllObjects();

	//Play a crate pickup sound.
	AudioEventRTS soundToPlay = TheAudio->getMiscAudio()->m_crateHeal;
	soundToPlay.setPosition( other->getPosition() );
	TheAudio->addAudioEvent(&soundToPlay);
	
	return TRUE;
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void HealCrateCollide::crc( Xfer *xfer )
{

	// extend base class
	CrateCollide::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void HealCrateCollide::xfer( Xfer *xfer )
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
void HealCrateCollide::loadPostProcess( void )
{

	// extend base class
	CrateCollide::loadPostProcess();

}  // end loadPostProcess

