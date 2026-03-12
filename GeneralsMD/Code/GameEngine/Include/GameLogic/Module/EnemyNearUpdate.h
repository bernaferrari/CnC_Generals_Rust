// FILE: EnemyNearUpdate.h /////////////////////////////////////////////////////////////////////////////
// Author: 	Matthew D. Campbell, April 2002
// Desc:  Reacts when an enemy is within range
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __EnemyNearUpdate_H_
#define __EnemyNearUpdate_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"
#include "Common/KindOf.h"

//-------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class EnemyNearUpdateModuleData : public UpdateModuleData
{
public:
	UnsignedInt m_enemyScanDelayTime;

	EnemyNearUpdateModuleData()
	{
		m_enemyScanDelayTime = LOGICFRAMES_PER_SECOND;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ "ScanDelayTime",		INI::parseDurationUnsignedInt,		NULL, offsetof( EnemyNearUpdateModuleData, m_enemyScanDelayTime ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
/** EnemyNear update */
//-------------------------------------------------------------------------------------------------
class EnemyNearUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( EnemyNearUpdate, "EnemyNearUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( EnemyNearUpdate, EnemyNearUpdateModuleData )

public:

	EnemyNearUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();

protected:

	UnsignedInt m_enemyScanDelay;
	Bool m_enemyNear;

	void checkForEnemies( void );

};

#endif // end __EnemyNearUpdate_H_

