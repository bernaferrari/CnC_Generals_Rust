// FILE: Damage.cpp ///////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2002
// Desc:   Basic structures for the damage process
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/Xfer.h"
#include "GameLogic/Damage.h"
#include "Common/BitFlagsIO.h"
#include "Common/ThingFactory.h"
#include "Common/ThingTemplate.h"

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

const char* DamageTypeFlags::s_bitNameList[] = 
{
	"EXPLOSION",			
	"CRUSH",					
	"ARMOR_PIERCING",
	"SMALL_ARMS",		
	"GATTLING",			
	"RADIATION",			
	"FLAME",					
	"LASER",					
	"SNIPER",				
	"POISON",			
	"HEALING",	
	"UNRESISTABLE",	
	"WATER",
	"DEPLOY",	
	"SURRENDER",	
	"HACK",	
	"KILL_PILOT",	
	"PENALTY",	
	"FALLING",	
	"MELEE",	
	"DISARM",	
	"HAZARD_CLEANUP",	
	"PARTICLE_BEAM",
	"TOPPLING",
	"INFANTRY_MISSILE",	
	"AURORA_BOMB",	
	"LAND_MINE",	
	"JET_MISSILES",	
	"STEALTHJET_MISSILES",	
	"MOLOTOV_COCKTAIL",	
	"COMANCHE_VULCAN",	
	"SUBDUAL_MISSILE",
	"SUBDUAL_VEHICLE",
	"SUBDUAL_BUILDING",
	"SUBDUAL_UNRESISTABLE",
	"MICROWAVE",
	"KILL_GARRISONED",
	"STATUS",

	NULL
};

DamageTypeFlags DAMAGE_TYPE_FLAGS_NONE; 	// inits to all zeroes
DamageTypeFlags DAMAGE_TYPE_FLAGS_ALL;

void initDamageTypeFlags()
{
	SET_ALL_DAMAGE_TYPE_BITS( DAMAGE_TYPE_FLAGS_ALL );
}

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void DamageInfo::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// xfer input
	xfer->xferSnapshot( &in );

	// xfer output
	xfer->xferSnapshot( &out );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version 
	* 2: Damage FX override
*/
// ------------------------------------------------------------------------------------------------
void DamageInfoInput::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 3;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// source id
	xfer->xferObjectID( &m_sourceID );

	// source player mask
	xfer->xferUser( &m_sourcePlayerMask, sizeof( PlayerMaskType ) );

	// damage type
	xfer->xferUser( &m_damageType, sizeof( DamageType ) );

	// damage FX Override
	if( version >= 2 )
		xfer->xferUser( &m_damageFXOverride, sizeof( DamageType ) );

	// death type
	xfer->xferUser( &m_deathType, sizeof( DeathType ) );

	// amount
	xfer->xferReal( &m_amount );

	// kill no matter what (old versions default to FALSE).
	if( currentVersion >= 2 )
	{
		xfer->xferBool( &m_kill );
	}

	xfer->xferUser( &m_damageStatusType, sizeof(ObjectStatusTypes) );//It's an enum

	xfer->xferCoord3D(&m_shockWaveVector);
	xfer->xferReal( &m_shockWaveAmount );
	xfer->xferReal( &m_shockWaveRadius );
	xfer->xferReal( &m_shockWaveTaperOff );

	if( version >= 3 )
	{
		AsciiString thingString = m_sourceTemplate ? m_sourceTemplate->getName() : AsciiString::TheEmptyString;
		xfer->xferAsciiString( &thingString );
		if( xfer->getXferMode() == XFER_LOAD )
		{
			m_sourceTemplate = TheThingFactory->findTemplate( thingString );
		}
	}

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void DamageInfoOutput::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// actual damage
	xfer->xferReal( &m_actualDamageDealt );

	// damage clipped
	xfer->xferReal( &m_actualDamageClipped );

	// no effect
	xfer->xferBool( &m_noEffect );

}  // end xfer

