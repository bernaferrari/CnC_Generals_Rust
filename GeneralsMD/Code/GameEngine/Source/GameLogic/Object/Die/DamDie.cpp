// FILE: DamDie.cpp ///////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, April 2002
// Desc:   The big water dam dying
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/RandomValue.h"
#include "Common/Xfer.h"
#include "GameClient/ParticleSys.h"
#include "GameClient/TerrainVisual.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/AIUpdate.h"
#include "GameLogic/Module/DamDie.h"

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
DamDieModuleData::DamDieModuleData( void )
{

}  // end DamDieModuleData

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
/*static*/ void DamDieModuleData::buildFieldParse(MultiIniFieldParse& p)
{

  DieModuleData::buildFieldParse( p );

//	static const FieldParse dataFieldParse[] = 
//	{
//		{ 0, 0, 0, 0 }
//	};
//
//  p.add(dataFieldParse);

}  // end buildFieldParse

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
DamDie::DamDie( Thing *thing, const ModuleData *moduleData )
			 :DieModule( thing, moduleData )
{

}  // end DamDie

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
DamDie::~DamDie( void )
{

}  // end ~DamDie

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void DamDie::onDie( const DamageInfo *damageInfo )
{
	if (!isDieApplicable(damageInfo))
		return;

	// enable all the water wave objects on the map
	Object *obj;
	for( obj = TheGameLogic->getFirstObject(); obj; obj = obj->getNextObject() )
	{

		// only care aboue water waves
		if( obj->isKindOf( KINDOF_WAVEGUIDE ) == FALSE )
			continue;

		// clear any disabled status of the water wave
		obj->clearDisabled( DISABLED_DEFAULT );

	}  // end for, obj

}  // end onDie

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void DamDie::crc( Xfer *xfer )
{

	// extend base class
	DieModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void DamDie::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	DieModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void DamDie::loadPostProcess( void )
{

	// extend base class
	DieModule::loadPostProcess();

}  // end loadPostProcess
