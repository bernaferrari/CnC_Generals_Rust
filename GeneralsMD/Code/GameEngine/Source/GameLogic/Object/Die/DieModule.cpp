// FILE: DieModule.cpp ////////////////////////////////////////////////////////////////////////////
// Author:
// Desc:   
///////////////////////////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#define DEFINE_OBJECT_STATUS_NAMES
#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameLogic/ExperienceTracker.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Module/DieModule.h"
#include "GameLogic/Object.h"


#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif



//-------------------------------------------------------------------------------------------------
DieMuxData::DieMuxData() :
	m_deathTypes(DEATH_TYPE_FLAGS_ALL),
	m_veterancyLevels(VETERANCY_LEVEL_FLAGS_ALL)
{
}

//-------------------------------------------------------------------------------------------------
const FieldParse* DieMuxData::getFieldParse() 
{
	static const FieldParse dataFieldParse[] = 
	{
		{ "DeathTypes",				INI::parseDeathTypeFlags,						NULL, offsetof( DieMuxData, m_deathTypes ) },
		{ "VeterancyLevels",	INI::parseVeterancyLevelFlags,			NULL, offsetof( DieMuxData, m_veterancyLevels ) },
		{ "ExemptStatus",			ObjectStatusMaskType::parseFromINI,	NULL,	offsetof( DieMuxData, m_exemptStatus ) },
		{ "RequiredStatus",		ObjectStatusMaskType::parseFromINI, NULL,	offsetof( DieMuxData, m_requiredStatus ) },
		{ 0, 0, 0, 0 }
	};
  return dataFieldParse;
}

//-------------------------------------------------------------------------------------------------
Bool DieMuxData::isDieApplicable(const Object* obj, const DamageInfo *damageInfo) const
{
	// wrong death type? punt
	if (!getDeathTypeFlag(m_deathTypes, damageInfo->in.m_deathType))
		return false;

	// wrong vet level? punt
	if (!getVeterancyLevelFlag(m_veterancyLevels, obj->getVeterancyLevel()))
		return false;

	// all 'exempt' bits must be clear for us to run.
	if( m_exemptStatus.any() && obj->getStatusBits().testForAny( m_exemptStatus ) )
		return false;

	// all 'required' bits must be set for us to run.
	// But only if we have a required status to check
	if( m_requiredStatus.any()  &&  !obj->getStatusBits().testForAll( m_requiredStatus ) )
		return false;

	return true;
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void DieModule::crc( Xfer *xfer )
{

	// extend base class
	BehaviorModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer Method */
// ------------------------------------------------------------------------------------------------
void DieModule::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// call base class
	BehaviorModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void DieModule::loadPostProcess( void )
{

	// call base class
	BehaviorModule::loadPostProcess();

}  // end loadPostProcess
