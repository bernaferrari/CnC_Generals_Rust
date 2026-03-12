// FILE: DieModule.h /////////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, September 2001
// Desc:	 
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __DieModule_H_
#define __DieModule_H_

#include "Common/Module.h"
#include "GameLogic/Damage.h"
#include "GameLogic/Module/BehaviorModule.h"
#include "Common/ObjectStatusTypes.h"

//-------------------------------------------------------------------------------------------------
/** OBJECT DIE MODULE base class */
//-------------------------------------------------------------------------------------------------


//-------------------------------------------------------------------------------------------------
class DieModuleInterface
{
public:
	virtual void onDie( const DamageInfo *damageInfo ) = 0;
};

//-------------------------------------------------------------------------------------------------
class DieMuxData	// does NOT inherit from ModuleData.
{
public:
	DeathTypeFlags				m_deathTypes;
	VeterancyLevelFlags		m_veterancyLevels;
	ObjectStatusMaskType	m_exemptStatus;						///< die module is ignored if any of these status bits are set
	ObjectStatusMaskType	m_requiredStatus;					///< die module is ignored if any of these status bits are clear

	DieMuxData();
	static const FieldParse* getFieldParse();

	Bool isDieApplicable(const Object* obj, const DamageInfo *damageInfo) const;
};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
class DieModuleData : public BehaviorModuleData
{
public:
	DieMuxData			m_dieMuxData;

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
		BehaviorModuleData::buildFieldParse(p);
		p.add(DieMuxData::getFieldParse(), offsetof( DieModuleData, m_dieMuxData ));
	}

	inline Bool isDieApplicable(const Object* obj, const DamageInfo *damageInfo) const { return m_dieMuxData.isDieApplicable(obj, damageInfo); }
};

//-------------------------------------------------------------------------------------------------
class DieModule : public BehaviorModule, public DieModuleInterface
{

	MEMORY_POOL_GLUE_ABC( DieModule )
	MAKE_STANDARD_MODULE_MACRO_ABC( DieModule )
	MAKE_STANDARD_MODULE_DATA_MACRO_ABC(DieModule, DieModuleData)

public:

	DieModule( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	static Int getInterfaceMask() { return MODULEINTERFACE_DIE; }

	// BehaviorModule
	virtual DieModuleInterface* getDie() { return this; }

	void onDie( const DamageInfo *damageInfo ) = 0;

protected:
	Bool isDieApplicable(const DamageInfo *damageInfo) const { return getDieModuleData()->isDieApplicable(getObject(), damageInfo); }
	
};
inline DieModule::DieModule( Thing *thing, const ModuleData* moduleData ) : BehaviorModule( thing, moduleData ) { }
inline DieModule::~DieModule() { }

#endif
