// FILE: BoneFXDamage.h /////////////////////////////////////////////////////////////////////
// Author: Bryan Cleveland, April 2002
// Desc:   Damage module for the boneFX update module
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __BONEFXDAMAGE_H_
#define __BONEFXDAMAGE_H_

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////

#include "GameLogic/Module/DamageModule.h" 

//#include "GameLogic/Module/BodyModule.h" -- Yikes... not necessary to include this! (KM)
enum BodyDamageType; //Ahhhh much better!


// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
class BoneFXDamage : public DamageModule
{

	MAKE_STANDARD_MODULE_MACRO( BoneFXDamage );
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( BoneFXDamage, "BoneFXDamage" )

public:

	BoneFXDamage( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	// damage module methods
	virtual void onDamage( DamageInfo *damageInfo ) { }
	virtual void onHealing( DamageInfo *damageInfo ) { }
	virtual void onBodyDamageStateChange( const DamageInfo* damageInfo, 
																				BodyDamageType oldState, 
																				BodyDamageType newState );

protected:

	virtual void onObjectCreated();

};

#endif  // end __BONEFXDAMAGE_H_
