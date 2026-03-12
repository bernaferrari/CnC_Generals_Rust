// FILE: RepairDockUpdate.h ///////////////////////////////////////////////////////////////////////
// Author: Colin Day, June 2002
// Desc:   The action of docking with a structure for repairs
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/BodyModule.h"
#include "GameLogic/Module/RepairDockUpdate.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
RepairDockUpdateModuleData::RepairDockUpdateModuleData( void )
{

	m_framesForFullHeal = 1.0f;  // 1 frame, instant heal by default (keeps away from divide by 0's)

}  // end RepairDockUpdateModuleData

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
/*static*/ void RepairDockUpdateModuleData::buildFieldParse(MultiIniFieldParse& p)
{

	DockUpdateModuleData::buildFieldParse( p );

	static const FieldParse dataFieldParse[] = 
	{
		{ "TimeForFullHeal", INI::parseDurationReal, NULL, offsetof( RepairDockUpdateModuleData, m_framesForFullHeal ) },
		{ 0, 0, 0, 0 }
	};

  p.add(dataFieldParse);

}  // end buildFieldParse

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
RepairDockUpdate::RepairDockUpdate( Thing *thing, const ModuleData* moduleData )
								: DockUpdate( thing, moduleData )
{

  m_lastRepair = INVALID_ID;
	m_healthToAddPerFrame = 0.0f;

}  // end RepairDockUpdate

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
RepairDockUpdate::~RepairDockUpdate( void )
{

}  // end ~RepairDockUpdate

// ------------------------------------------------------------------------------------------------
/** Do the action while docked
	* Return TRUE to continue the docking process
	* Return FALSE to complete the dockin process */
// ------------------------------------------------------------------------------------------------
Bool RepairDockUpdate::action( Object *docker, Object *drone )
{

	// sanity
	if( docker == NULL )
		return FALSE;

	// get our module data
	const RepairDockUpdateModuleData *modData = getRepairDockUpdateModuleData();

	// get the body module for the docker
	BodyModuleInterface *body = docker->getBodyModule();

	//
	// no matter if the object is damaged just a little bit, or on the brink of death, we will
	// heal the object over a fixed amount of *TIME* that is specified in the INI file.  Whenever
	// we get a new docker, given its current health we figure out how much health we will add
	// to this docked object each frame so that it is fully healed after the correct amount
	// of time has passed
	//
	if( m_lastRepair == 0 )
	{

		// save ID of this docker as the last docker
		m_lastRepair = docker->getID();

		//
		// figure out how much health we need to add each frame to this object so that it's
		// fully healed at the right time
		//
		m_healthToAddPerFrame = (body->getMaxHealth() - body->getHealth()) / modData->m_framesForFullHeal;

	}  // end if

	// if we're at max health we're done
	if( body->getHealth() >= body->getMaxHealth() )
	{

		// repair is complete, clear our last docker
		m_lastRepair = INVALID_ID;

		// returning false will complete the docking process
		return FALSE;

	}  // end if

	// give us some health buddy
	DamageInfo healingInfo;
	healingInfo.in.m_amount = m_healthToAddPerFrame;
	healingInfo.in.m_sourceID = getObject()->getID();
	healingInfo.in.m_damageType = DAMAGE_HEALING;
	healingInfo.in.m_deathType = DEATH_NONE;
	body->attemptHealing( &healingInfo );
	if( drone )
	{
		body = drone->getBodyModule();
		healingInfo.in.m_amount = body->getMaxHealth();
		body->attemptHealing( &healingInfo );
	}
	
	// stay docked
	return TRUE;

}  // end action

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void RepairDockUpdate::crc( Xfer *xfer )
{

	// extend base class
	DockUpdate::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void RepairDockUpdate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	DockUpdate::xfer( xfer );

	// last repair
	xfer->xferObjectID( &m_lastRepair );

	// health to add per frame
	xfer->xferReal( &m_healthToAddPerFrame );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void RepairDockUpdate::loadPostProcess( void )
{

	// extend base class
	DockUpdate::loadPostProcess();

}  // end loadPostProcess
