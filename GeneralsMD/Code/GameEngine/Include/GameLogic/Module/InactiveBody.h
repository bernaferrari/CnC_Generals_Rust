// FILE: InactiveBody.h ///////////////////////////////////////////////////////////////////////////
// Author: Colin Day, November 2001
// Desc:	 An inactive body module, they are indestructable and largely cannot be
//				 affected by things in the world.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __INACTIVEBODY_H_
#define __INACTIVEBODY_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/BodyModule.h"

//-------------------------------------------------------------------------------------------------
/** Inactive body module */
//-------------------------------------------------------------------------------------------------
class InactiveBody : public BodyModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( InactiveBody, "InactiveBody" )
	MAKE_STANDARD_MODULE_MACRO( InactiveBody )

public:

	InactiveBody( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void attemptDamage( DamageInfo *damageInfo );		///< try to damage this object
	virtual void attemptHealing( DamageInfo *damageInfo );		///< try to heal this object
	virtual Real estimateDamage( DamageInfoInput& damageInfo ) const;
	virtual Real getHealth() const;													///< get current health
	virtual BodyDamageType getDamageState() const;
	virtual void setDamageState( BodyDamageType newState );	///< control damage state directly.  Will adjust hitpoints.
	virtual void setAflame( Bool setting ){}///< This is a major change like a damage state.  

	void onVeterancyLevelChanged( VeterancyLevel oldLevel, VeterancyLevel newLevel, Bool provideFeedback ) { /* nothing */ }

	virtual void setArmorSetFlag(ArmorSetType ast) { /* nothing */ }
	virtual void clearArmorSetFlag(ArmorSetType ast) { /* nothing */ }
	virtual Bool testArmorSetFlag(ArmorSetType ast){ return FALSE; }

	virtual void internalChangeHealth( Real delta );

private:
	Bool m_dieCalled;
};

#endif // __INACTIVEBODY_H_

