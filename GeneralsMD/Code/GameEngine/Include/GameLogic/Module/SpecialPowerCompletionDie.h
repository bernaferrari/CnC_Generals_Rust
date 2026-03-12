// FILE: SpecialPowerCompletionDie.h //////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, May 2002
// Desc:   Die method responsible for telling TheScriptEngine that a special power has been completed
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _SPECIAL_POWER_COMPLETION_DIE_H_
#define _SPECIAL_POWER_COMPLETION_DIE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/INI.h"
#include "GameLogic/Module/DieModule.h"

class SpecialPowerTemplate;

//-------------------------------------------------------------------------------------------------
class SpecialPowerCompletionDieModuleData : public DieModuleData
{
public:
	SpecialPowerTemplate *m_specialPowerTemplate;		///< pointer to the special power template

	SpecialPowerCompletionDieModuleData()
	{
		m_specialPowerTemplate = NULL;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    DieModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "SpecialPowerTemplate", INI::parseSpecialPowerTemplate,	NULL, offsetof( SpecialPowerCompletionDieModuleData, m_specialPowerTemplate ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);

	}
};

//-------------------------------------------------------------------------------------------------
/** Special power completion die */
//-------------------------------------------------------------------------------------------------
class SpecialPowerCompletionDie : public DieModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SpecialPowerCompletionDie, "SpecialPowerCompletionDie" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SpecialPowerCompletionDie, SpecialPowerCompletionDieModuleData )

public:

	SpecialPowerCompletionDie( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	void setCreator( ObjectID creatorID );
	void notifyScriptEngine( void );

	virtual void onDie( const DamageInfo *damageInfo ); 

protected:

	ObjectID m_creatorID;
	Bool m_creatorSet;

};

#endif // _SPECIAL_POWER_COMPLETION_DIE_H_

// Creator is stored as ID, so a failed lookup just means that he died first and noone cares that we are going.