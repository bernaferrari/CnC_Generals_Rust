// FILE: TechBuildingBehavior.cpp /////////////////////////////////////////////////////////////////
// Author: Colin Day, October 2002
// Desc:   Tech building basic behavior
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.H"
#include "Common/Player.h"
#include "Common/PlayerList.h"
#include "Common/ThingTemplate.h"
#include "Common/Xfer.h"
#include "GameClient/FXList.h"
#include "GameClient/InGameUI.h"
#include "GameLogic/Module/TechBuildingBehavior.h"
#include "GameLogic/Object.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
TechBuildingBehaviorModuleData::TechBuildingBehaviorModuleData( void )
{
	m_pulseFX = NULL;
	m_pulseFXRate = 0;
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
/*static*/ void TechBuildingBehaviorModuleData::buildFieldParse( MultiIniFieldParse &p )
{
  UpdateModuleData::buildFieldParse( p );

	static const FieldParse dataFieldParse[] = 
	{
		{ "PulseFX",								INI::parseFXList,								NULL,	offsetof( TechBuildingBehaviorModuleData, m_pulseFX ) },
		{ "PulseFXRate",						INI::parseDurationUnsignedInt,	NULL,	offsetof( TechBuildingBehaviorModuleData, m_pulseFXRate ) },
		{ 0, 0, 0, 0 }
	};

  p.add( dataFieldParse );

}  // end buildFieldParse

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
TechBuildingBehavior::TechBuildingBehavior( Thing *thing, const ModuleData *modData )
										: UpdateModule( thing, modData )
{

	//
	// setup ourselves so we do at least one update evaluation after the module
	// is in the world
	//
	setWakeFrame(getObject(), UPDATE_SLEEP_NONE);

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
TechBuildingBehavior::~TechBuildingBehavior( void )
{
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
UpdateSleepTime TechBuildingBehavior::update( void )
{
	Object *us = getObject();
	const TechBuildingBehaviorModuleData* d = getTechBuildingBehaviorModuleData();
	Bool captured = false;
	
	// update our model condition for the captured status
	Player *player = us->getControllingPlayer();
	if( player && player->isPlayableSide() )
	{
		us->setModelConditionState( MODELCONDITION_CAPTURED );
		captured = true;
	}
	else
	{
		us->clearModelConditionState( MODELCONDITION_CAPTURED );
		captured = false;
	}

	// if we have a pulse fx, and are owned, sleep only a little while, otherwise sleep forever
	if (d->m_pulseFX != NULL && d->m_pulseFXRate > 0 && captured)
	{
		FXList::doFXObj( d->m_pulseFX, us );
		return UPDATE_SLEEP(d->m_pulseFXRate);
	}
	else
	{
		// now sleep forever my dear
		return UPDATE_SLEEP_FOREVER;
	}
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void TechBuildingBehavior::onDie( const DamageInfo *damageInfo )
{

	//
	// put us on the team of the neutral player so no player has any bonus from us
	//
	Object *us = getObject();
	us->clearModelConditionState( MODELCONDITION_CAPTURED );
	us->setTeam( ThePlayerList->getNeutralPlayer()->getDefaultTeam() );

}  // end onDie

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void TechBuildingBehavior::onCapture( Player *oldOwner, Player *newOwner )
{

	// wake up next frame so we can re-evaluate our captured status
	setWakeFrame( getObject(), UPDATE_SLEEP_NONE );

}  // end onCapture

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void TechBuildingBehavior::crc( Xfer *xfer )
{

	// extend base class
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void TechBuildingBehavior::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );
	
	// extend base class
	UpdateModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void TechBuildingBehavior::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}  // end loadPostProcess

