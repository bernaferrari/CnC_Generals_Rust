// FILE: RadarUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: Colin Day, April 2002
// Desc:   Updating a radar on an object
///////////////////////////////////////////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/ModelState.h"
#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/RadarUpdate.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
RadarUpdateModuleData::RadarUpdateModuleData( void )
{

	m_radarExtendTime = 0.0f;

}  // end RadarUpdateModuleData

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
RadarUpdate::RadarUpdate( Thing *thing, const ModuleData *moduleData )
												: UpdateModule( thing, moduleData )
{

	m_radarActive = FALSE;
	m_extendDoneFrame = 0;
	m_extendComplete = FALSE;

}  // end RadarUpdate

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
RadarUpdate::~RadarUpdate( void )
{

}  // end RadarUpdate

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void RadarUpdate::extendRadar( void )
{
	const RadarUpdateModuleData *modData = getRadarUpdateModuleData();

	// set the model condition for radar extension
	Drawable *draw = getObject()->getDrawable();
	if( draw )
		draw->setModelConditionState( MODELCONDITION_RADAR_EXTENDING );

	// mark the frame that the extension will be done on
	m_extendDoneFrame = TheGameLogic->getFrame() + modData->m_radarExtendTime;

	//Change this to make the radar active after extension...
	m_radarActive = true;

}  // end extendRadar

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UpdateSleepTime RadarUpdate::update( void )
{
/// @todo srj use SLEEPY_UPDATE here

	// if no extend frame nothing to do
	if( m_extendDoneFrame == 0 )
		return UPDATE_SLEEP_NONE;

	// check to see if our extension is already done
	if( m_extendComplete == TRUE )
		return UPDATE_SLEEP_NONE;

	// see if it's time to stop the extension
	if( TheGameLogic->getFrame() > m_extendDoneFrame )
	{

		// mark extend as done
		m_extendComplete = TRUE;
		m_extendDoneFrame = 0;  // just to be clean

		// remove the extending condition and set the extened condition
		Drawable *draw = getObject()->getDrawable();
		if( draw )
			draw->clearAndSetModelConditionState( MODELCONDITION_RADAR_EXTENDING,
																						MODELCONDITION_RADAR_UPGRADED );

	}  // end if
	
	return UPDATE_SLEEP_NONE;

}  // end update

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void RadarUpdate::crc( Xfer *xfer )
{

	// extend base class
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void RadarUpdate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpdateModule::xfer( xfer );

	// extend done frame
	xfer->xferUnsignedInt( &m_extendDoneFrame );

	// extend complete
	xfer->xferBool( &m_extendComplete );

	// radar active
	xfer->xferBool( &m_radarActive );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void RadarUpdate::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}  // end loadPostProcess

