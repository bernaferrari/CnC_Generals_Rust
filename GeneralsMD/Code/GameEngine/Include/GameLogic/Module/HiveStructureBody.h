// FILE: HiveStructureBody.h //////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, October 2002
// Desc:	 Hive structure bodies are structure bodies with the ability to propagate specified
//         damage types to slaves when available. If there are no slaves, the the structure
//         will take the damage.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __HIVE_STRUCTURE_BODY_H
#define __HIVE_STRUCTURE_BODY_H

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/StructureBody.h"
#include "GameLogic/Damage.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Object;

class HiveStructureBodyModuleData : public StructureBodyModuleData
{
public:
	DamageTypeFlags m_damageTypesToPropagateToSlaves;
	DamageTypeFlags m_damageTypesToSwallow;							///< A subset of the damage types to propagate. Do not take them ourselves

	HiveStructureBodyModuleData();

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    StructureBodyModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "PropagateDamageTypesToSlavesWhenExisting",   INI::parseDamageTypeFlags, NULL, offsetof( HiveStructureBodyModuleData, m_damageTypesToPropagateToSlaves ) },
			{ "SwallowDamageTypesIfSlavesNotExisting",			INI::parseDamageTypeFlags, NULL, offsetof( HiveStructureBodyModuleData, m_damageTypesToSwallow ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};


//-------------------------------------------------------------------------------------------------
/** Structure body module */
//-------------------------------------------------------------------------------------------------
class HiveStructureBody : public StructureBody
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( HiveStructureBody, "HiveStructureBody" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( HiveStructureBody, HiveStructureBodyModuleData )

public:

	HiveStructureBody( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

protected:

	virtual void attemptDamage( DamageInfo *damageInfo );		///< try to damage this object
};

#endif // __HIVE_STRUCTURE_BODY_H

