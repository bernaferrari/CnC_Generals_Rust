// FILE: PowerPlantUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: Amit Kumar, August 2002
// Desc:   Updating the power plant
///////////////////////////////////////////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/ModelState.h"
#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameClient/InGameUI.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/PowerPlantUpdate.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
PowerPlantUpdateModuleData::PowerPlantUpdateModuleData( void )
{

	m_rodsExtendTime = 0;

}  // end PowerPlantUpdateModuleData

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
PowerPlantUpdate::PowerPlantUpdate( Thing *thing, const ModuleData *moduleData )
												: UpdateModule( thing, moduleData )
{

	m_extended = FALSE;
	setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER);

}  // end PowerPlantUpdate

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
PowerPlantUpdate::~PowerPlantUpdate( void )
{

}  // end PowerPlantUpdate

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void PowerPlantUpdate::extendRods( Bool extend )
{
	if (extend)
	{
		if (!m_extended)
		{
			const PowerPlantUpdateModuleData *modData = getPowerPlantUpdateModuleData();

			// set the model condition for radar extension
			Drawable *draw = getObject()->getDrawable();
			if( draw )
				draw->setModelConditionState( MODELCONDITION_POWER_PLANT_UPGRADING );

			m_extended = TRUE;
			setWakeFrame(getObject(), UPDATE_SLEEP(modData->m_rodsExtendTime));
		}
	}
	else
	{
		// they de-extend instantly.
		Drawable *draw = getObject()->getDrawable();
		if( draw )
			draw->clearModelConditionFlags( MAKE_MODELCONDITION_MASK2( MODELCONDITION_POWER_PLANT_UPGRADING, 
																														MODELCONDITION_POWER_PLANT_UPGRADED) );
		
		m_extended = FALSE;
		setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER);
	}

}  // end PowerPlantUpdate

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UpdateSleepTime PowerPlantUpdate::update( void )
{
	// remove the extending condition and set the extened condition
	Drawable *draw = getObject()->getDrawable();
	if( draw )
		draw->clearAndSetModelConditionState( MODELCONDITION_POWER_PLANT_UPGRADING,
																					MODELCONDITION_POWER_PLANT_UPGRADED );
	
	m_extended = TRUE;
	return UPDATE_SLEEP_FOREVER;
}  // end update

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void PowerPlantUpdate::crc( Xfer *xfer )
{

	// extend base class
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void PowerPlantUpdate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpdateModule::xfer( xfer );

	// extend complete
	xfer->xferBool( &m_extended );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void PowerPlantUpdate::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}  // end loadPostProcess
