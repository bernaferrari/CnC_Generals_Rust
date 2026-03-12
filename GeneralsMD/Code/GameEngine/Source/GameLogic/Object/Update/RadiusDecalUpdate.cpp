// FILE: RadiusDecalUpdate.cpp ///////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/RandomValue.h"
#include "Common/Xfer.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Module/RadiusDecalUpdate.h"
#include "GameLogic/Object.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
RadiusDecalUpdate::RadiusDecalUpdate( Thing *thing, const ModuleData* moduleData ) : UpdateModule( thing, moduleData )
{
	m_deliveryDecal.clear();
	m_killWhenNoLongerAttacking = false;
	setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
RadiusDecalUpdate::~RadiusDecalUpdate( void )
{
	m_deliveryDecal.clear();
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void RadiusDecalUpdate::createRadiusDecal( const RadiusDecalTemplate& tmpl, Real radius, const Coord3D& pos )
{
	m_deliveryDecal.clear();
	tmpl.createRadiusDecal(pos, radius, getObject()->getControllingPlayer(), m_deliveryDecal);
	setWakeFrame(getObject(), m_deliveryDecal.isEmpty() ? UPDATE_SLEEP_FOREVER : UPDATE_SLEEP_NONE);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void RadiusDecalUpdate::killRadiusDecal()
{
	m_deliveryDecal.clear();
	setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UpdateSleepTime RadiusDecalUpdate::update( void )
{
	if (m_killWhenNoLongerAttacking && !getObject()->testStatus( OBJECT_STATUS_IS_ATTACKING ))
	{
		m_deliveryDecal.clear();
		return UPDATE_SLEEP_FOREVER;
	}

	m_deliveryDecal.update();
	return UPDATE_SLEEP_NONE;
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void RadiusDecalUpdate::crc( Xfer *xfer )
{

	// extend base class
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void RadiusDecalUpdate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpdateModule::xfer( xfer );

	// decal, if any
	m_deliveryDecal.xferRadiusDecal(xfer);

	xfer->xferBool(&m_killWhenNoLongerAttacking);

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void RadiusDecalUpdate::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}  // end loadPostProcess
