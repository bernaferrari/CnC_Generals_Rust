// FILE: UndeadBody.h ////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, June 2003
// Desc:	 First death is intercepted and sets flags and setMaxHealth.  Second death is handled normally.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __UNDEAD_BODY_H
#define __UNDEAD_BODY_H

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ActiveBody.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Object;

//-------------------------------------------------------------------------------------------------
class UndeadBodyModuleData : public ActiveBodyModuleData 
{
public:
	Real m_secondLifeMaxHealth;

	UndeadBodyModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class UndeadBody : public ActiveBody
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( UndeadBody, "UndeadBody" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( UndeadBody, UndeadBodyModuleData )

public:

	UndeadBody( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void attemptDamage( DamageInfo *damageInfo );		///< try to damage this object

protected:

	Bool m_isSecondLife;	/** This is false until I detect death the first time, then I 
														change my Max, Initial, and Current health and stop intercepting anything.
												*/
	void startSecondLife(DamageInfo *damageInfo);

};

#endif 

