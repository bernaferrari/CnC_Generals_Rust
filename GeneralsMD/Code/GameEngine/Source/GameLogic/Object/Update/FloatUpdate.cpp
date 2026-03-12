// FILE: FloatUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: Colin Day, May 2002
// Desc:   Float on top of da water!!!
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/TerrainLogic.h"
#include "GameLogic/Module/FloatUpdate.h"
#include "GameLogic/GameLogic.h"

#include "GameClient/Drawable.h"



#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif


// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
FloatUpdateModuleData::FloatUpdateModuleData( void )
{

	m_enabled = FALSE;

}  // end FloatUpdateModuleData

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
/*static*/ void FloatUpdateModuleData::buildFieldParse(MultiIniFieldParse& p)
{

	UpdateModuleData::buildFieldParse( p );

	static const FieldParse dataFieldParse[] = 
	{
		{ "Enabled",	INI::parseBool,	NULL, offsetof( FloatUpdateModuleData, m_enabled ) },
		{ 0, 0, 0, 0 }
	};
	p.add(dataFieldParse);

}  // end buildFieldParse

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
FloatUpdate::FloatUpdate( Thing *thing, const ModuleData *moduleData )
						:UpdateModule( thing, moduleData )
{

	// save our initial enabled status based on INI settings
	m_enabled = ((FloatUpdateModuleData *)moduleData)->m_enabled;

}  // end FloatUpdate

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
FloatUpdate::~FloatUpdate( void )
{

}  // end ~FloatUpdate

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
UpdateSleepTime FloatUpdate::update( void )
{
/// @todo srj use SLEEPY_UPDATE here

	// if we're not enabled, do nothing
	if( m_enabled == TRUE )
	{
		// get object position
		const Coord3D *pos = getObject()->getPosition();
		
		// get the height of the water here
		Real waterZ;
		TheTerrainLogic->isUnderwater( pos->x, pos->y, &waterZ );

		// snap to the water surface
		Coord3D newPos;
		newPos.x = pos->x;
		newPos.y = pos->y;
		newPos.z = waterZ;
		getObject()->setPosition( &newPos );

	}

	Drawable *draw = getObject()->getDrawable();
	if (draw)
	{

		Real angle = INT_TO_REAL(TheGameLogic->getFrame());
		Real yaw = sin(angle * 0.0291f) * 0.05f;
		Real pitch = sin(angle * 0.0515f) * 0.05f;

		Matrix3D mx = *draw->getInstanceMatrix();

		Real zRot = mx.Get_Z_Rotation();
		mx.Make_Identity();
		mx.Rotate_Z(zRot);
		mx.Rotate_Y(yaw);
		mx.Rotate_X(pitch);
		
		draw->setInstanceMatrix(&mx);
	}

	return UPDATE_SLEEP_NONE;
}  // end update

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void FloatUpdate::crc( Xfer *xfer )
{

	// extend base class
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void FloatUpdate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpdateModule::xfer( xfer );

	// enabled
	xfer->xferBool( &m_enabled );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void FloatUpdate::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}  // end loadPostProcess
