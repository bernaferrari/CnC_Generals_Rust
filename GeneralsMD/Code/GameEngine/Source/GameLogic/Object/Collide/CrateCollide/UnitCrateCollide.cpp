// FILE: UnitCrateCollide.cpp ///////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, March 2002
// Desc:   A crate that gives n units of type m the the pickerupper
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/AudioEventRTS.h"
#include "Common/MiscAudio.h"
#include "Common/Player.h"
#include "Common/ThingFactory.h"
#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/PartitionManager.h"
#include "GameLogic/Module/UnitCrateCollide.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UnitCrateCollide::UnitCrateCollide( Thing *thing, const ModuleData* moduleData ) : CrateCollide( thing, moduleData )
{

} 

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UnitCrateCollide::~UnitCrateCollide( void )
{

}  

//-------------------------------------------------------------------------------------------------
Bool UnitCrateCollide::executeCrateBehavior( Object *other )
{
	UnsignedInt unitCount = getUnitCrateCollideModuleData()->m_unitCount;
	ThingTemplate const *unitType = TheThingFactory->findTemplate( getUnitCrateCollideModuleData()->m_unitType );

	if( unitType == NULL )
	{
		return FALSE;
	}

	for( Int unitIndex = 0; unitIndex < unitCount; unitIndex++ )
	{
		Team *creationTeam = other->getControllingPlayer()->getDefaultTeam();
		Object *newObj = TheThingFactory->newObject( unitType, creationTeam );
		if( newObj )
		{
			Coord3D creationPoint = *other->getPosition();
			/// @todo As a user of the future findLegalPositionAround, I wouldn't mind not having to specify range.  I just want a non colliding point.
			FindPositionOptions fpOptions;
			fpOptions.minRadius = 0.0f;
			fpOptions.maxRadius = 20.0f;
			ThePartitionManager->findPositionAround( &creationPoint,
																							 &fpOptions,
																							 &creationPoint );

			newObj->setOrientation( other->getOrientation() );
			newObj->setPosition( &creationPoint );
		} 
	}

	//Play a crate pickup sound.
	AudioEventRTS soundToPlay = TheAudio->getMiscAudio()->m_crateFreeUnit;
	soundToPlay.setObjectID( other->getID() );
	TheAudio->addAudioEvent(&soundToPlay);

	return TRUE;
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void UnitCrateCollide::crc( Xfer *xfer )
{

	// extend base class
	CrateCollide::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void UnitCrateCollide::xfer( Xfer *xfer )
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
void UnitCrateCollide::loadPostProcess( void )
{

	// extend base class
	CrateCollide::loadPostProcess();

}  // end loadPostProcess
