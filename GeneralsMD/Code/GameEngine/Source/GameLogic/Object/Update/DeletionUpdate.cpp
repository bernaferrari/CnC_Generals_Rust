// FILE: DeletionUpdate.cpp /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, August 2002
// Desc:   Update that will count down a lifetime and destroy object when it reaches zero
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/RandomValue.h"
#include "Common/Xfer.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/DeletionUpdate.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
DeletionUpdate::DeletionUpdate( Thing *thing, const ModuleData* moduleData ) : UpdateModule( thing, moduleData )
{
	m_dieFrame = 0;
	const DeletionUpdateModuleData* d = getDeletionUpdateModuleData();
	UnsignedInt delay = calcSleepDelay(d->m_minFrames, d->m_maxFrames);
	setWakeFrame(getObject(), UPDATE_SLEEP(delay));
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
DeletionUpdate::~DeletionUpdate( void )
{
}


//#define CRISS_CROSS_GEOMETRY

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void DeletionUpdate::setLifetimeRange( UnsignedInt minFrames, UnsignedInt maxFrames )
{
	
#if defined _DEBUG && defined CRISS_CROSS_GEOMETRY
	setWakeFrame(getObject(), UPDATE_SLEEP(2));
#else
	UnsignedInt delay = calcSleepDelay(minFrames, maxFrames);
	setWakeFrame(getObject(), UPDATE_SLEEP(delay));
#endif
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UnsignedInt DeletionUpdate::calcSleepDelay(UnsignedInt minFrames, UnsignedInt maxFrames)
{
	UnsignedInt delay = GameLogicRandomValue( minFrames, maxFrames );
	if (delay < 1) delay = 1;
	m_dieFrame = TheGameLogic->getFrame() + delay;
	return delay;
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UpdateSleepTime DeletionUpdate::update( void )
{
	// Destroy (NOT kill) if time is up
#if defined _DEBUG  && defined CRISS_CROSS_GEOMETRY
	Object *obj = getObject();
	if (obj)
	{
		GeometryInfo geom =	geom=obj->getGeometryInfo();
		geom.setMajorRadius(obj->getGeometryInfo().getMinorRadius());// CRIS
		geom.setMinorRadius(obj->getGeometryInfo().getMajorRadius());// CROSS
		obj->setGeometryInfo(geom);
	}

	if (TheGameLogic->getFrame() > m_dieFrame)
	{
		TheGameLogic->destroyObject( getObject() );
		return UPDATE_SLEEP_FOREVER;
	}


	return UPDATE_SLEEP(2);
#else
	TheGameLogic->destroyObject( getObject() );
	return UPDATE_SLEEP_FOREVER;
#endif
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void DeletionUpdate::crc( Xfer *xfer )
{

	// extend base class
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void DeletionUpdate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpdateModule::xfer( xfer );

	// die frame
	xfer->xferUnsignedInt( &m_dieFrame );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void DeletionUpdate::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}  // end loadPostProcess
