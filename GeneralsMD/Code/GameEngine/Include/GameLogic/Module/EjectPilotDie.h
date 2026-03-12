// FILE: EjectPilotDie.h /////////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, April 2002
// Desc:   Create object at current object's death
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _EjectPilotDie_H_
#define _EjectPilotDie_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/DieModule.h"
#include "Common/INI.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class ObjectCreationList;

//-------------------------------------------------------------------------------------------------
class EjectPilotDieModuleData : public DieModuleData
{
public:
	const ObjectCreationList* m_oclInAir;
	const ObjectCreationList* m_oclOnGround;
	UnsignedInt m_invulnerableTime;

	EjectPilotDieModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

//-------------------------------------------------------------------------------------------------
/** When this object dies, create another object in its place */
//-------------------------------------------------------------------------------------------------
class EjectPilotDie : public DieModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( EjectPilotDie, "EjectPilotDie"  )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( EjectPilotDie, EjectPilotDieModuleData );

public:

	EjectPilotDie( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	static void ejectPilot(const ObjectCreationList* ocl, const Object* dyingObject, const Object* damageDealer);

	virtual void onDie( const DamageInfo *damageInfo ); 
	virtual DieModuleInterface* getEjectPilotDieInterface( void ) {return this; }

};

#endif // _EjectPilotDie_H_

