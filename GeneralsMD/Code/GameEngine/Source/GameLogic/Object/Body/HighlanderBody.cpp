// FILE: HighlanderBody.cpp ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, November 2002
// Desc:	 Takes damage according to armor, but can't die from normal damage.  Can die from Unresistable though
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine
#include "Common/Xfer.h"

#include "GameLogic/Module/HighlanderBody.h"

// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
HighlanderBody::HighlanderBody( Thing *thing, const ModuleData* moduleData ) 
						 : ActiveBody( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
HighlanderBody::~HighlanderBody( void )
{

}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void HighlanderBody::attemptDamage( DamageInfo *damageInfo )
{
	// Bind to one hitpoint remaining afterwards, unless it is Unresistable damage
	if( damageInfo->in.m_damageType != DAMAGE_UNRESISTABLE )
		damageInfo->in.m_amount = min( damageInfo->in.m_amount, getHealth() - 1 );

	ActiveBody::attemptDamage(damageInfo);
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void HighlanderBody::crc( Xfer *xfer )
{

	// extend base class
	ActiveBody::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void HighlanderBody::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	ActiveBody::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void HighlanderBody::loadPostProcess( void )
{

	// extend base class
	ActiveBody::loadPostProcess();

}  // end loadPostProcess
