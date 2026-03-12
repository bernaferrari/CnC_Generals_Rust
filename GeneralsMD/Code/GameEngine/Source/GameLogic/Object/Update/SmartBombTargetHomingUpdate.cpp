// FILE: SmartBombTargetHomingUpdate.cpp /////////////////////////////////////////////////////////////////////////
// Author: Mark Lorenzen, July 2003
// Desc:   Update that will fudge a falling object's position just slightly, to make it find its target better
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/RandomValue.h"
#include "Common/Xfer.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/SmartBombTargetHomingUpdate.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SmartBombTargetHomingUpdate::SmartBombTargetHomingUpdate( Thing *thing, const ModuleData* moduleData ) : UpdateModule( thing, moduleData )
{
  m_targetReceived = FALSE;
  m_target.zero();
	setWakeFrame( getObject(), UPDATE_SLEEP_NONE );
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SmartBombTargetHomingUpdate::~SmartBombTargetHomingUpdate( void )
{
}
//#define CRISS_CROSS_GEOMETRY


//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void SmartBombTargetHomingUpdate::SetTargetPosition( const Coord3D& target )
{

  // Ensure that we have been passed a real-world location
  DEBUG_ASSERTCRASH( target.length() > 0.0f, ("SmartBombTargetHomingUpdate::SetTargetPosition() received a zero coord") );
  if ( ! (target.length() > 0.0f) )
    return;

  m_target.x = target.x;
  m_target.y = target.y;
  m_target.z = target.z;

  m_targetReceived = TRUE;

}


//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UpdateSleepTime SmartBombTargetHomingUpdate::update( void )
{

  if ( ! m_targetReceived )
    return UPDATE_SLEEP_NONE;

	const SmartBombTargetHomingUpdateModuleData* d = getSmartBombTargetHomingUpdateModuleData();
  if ( ! d )
    return UPDATE_SLEEP_NONE;

  Object *self = getObject();
  if ( ! self )
    return UPDATE_SLEEP_NONE;


  if ( ! self->isSignificantlyAboveTerrain() )
    return UPDATE_SLEEP_NONE;

  const Coord3D *currentPos = self->getPosition();

  Coord3D pos;
  pos.zero();

  Real statusCoeff = MAX( 0.0f, MIN( 1.0f, d->m_courseCorrectionScalar));
  Real targetCoeff = 1.0f - statusCoeff;


  pos.x = m_target.x * targetCoeff + currentPos->x * statusCoeff;
  pos.y = m_target.y * targetCoeff + currentPos->y * statusCoeff;
  pos.z = currentPos->z;

  self->setPosition( &pos );

  return UPDATE_SLEEP_NONE;

}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void SmartBombTargetHomingUpdate::crc( Xfer *xfer )
{

	// extend base class
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void SmartBombTargetHomingUpdate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpdateModule::xfer( xfer );


}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void SmartBombTargetHomingUpdate::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}  // end loadPostProcess
