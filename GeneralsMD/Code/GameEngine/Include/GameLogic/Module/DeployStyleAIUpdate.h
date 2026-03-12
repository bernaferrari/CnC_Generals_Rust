// DeployStyleAIUpdate.h ////////////
// Author: Kris Morness, August 2002
// Desc:   State machine that allows deploying/undeploying to control the AI. 
//         When deployed, you can't move, when undeployed, you can't attack.

#pragma once

#ifndef __DEPLOY_STYLE_AI_UPDATE_H
#define __DEPLOY_STYLE_AI_UPDATE_H

#include "Common/StateMachine.h"
#include "GameLogic/Module/AIUpdate.h"

//-------------------------------------------------------------------------------------------------
enum DeployStateTypes
{
	READY_TO_MOVE,							///< Mobile, can't attack.
	DEPLOY,											///< Not mobile, can't attack, currently unpacking to attack
	READY_TO_ATTACK,						///< Not mobile, can attack
	UNDEPLOY,										///< Not mobile, can't attack, currently packing to move
	ALIGNING_TURRETS,						///< While deployed, we must wait for the turret to go back to natural position prior to undeploying.
};

//-------------------------------------------------------------------------------------------------
class DeployStyleAIUpdateModuleData : public AIUpdateModuleData
{
public:
	UnsignedInt			m_unpackTime;
	UnsignedInt			m_packTime;		
	Bool						m_resetTurretBeforePacking;	
	Bool						m_turretsFunctionOnlyWhenDeployed;
	Bool						m_turretsMustCenterBeforePacking;
	Bool						m_manualDeployAnimations;

	DeployStyleAIUpdateModuleData()
	{
		m_unpackTime = 0;
		m_packTime = 0;
		m_resetTurretBeforePacking = false;
		m_turretsFunctionOnlyWhenDeployed = false;
		// Added By Sadullah Nader
		// Initialization necessary 
		m_turretsMustCenterBeforePacking = FALSE;
		// End Add
		m_manualDeployAnimations = FALSE;
	}

	static void buildFieldParse(MultiIniFieldParse& p)
	{
		AIUpdateModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "UnpackTime",					INI::parseDurationUnsignedInt,	NULL, offsetof( DeployStyleAIUpdateModuleData, m_unpackTime ) },
			{ "PackTime",						INI::parseDurationUnsignedInt,	NULL, offsetof( DeployStyleAIUpdateModuleData, m_packTime ) },
			{ "ResetTurretBeforePacking", INI::parseBool,						NULL, offsetof( DeployStyleAIUpdateModuleData, m_resetTurretBeforePacking ) },
			{ "TurretsFunctionOnlyWhenDeployed", INI::parseBool,		NULL, offsetof( DeployStyleAIUpdateModuleData, m_turretsFunctionOnlyWhenDeployed ) },
			{ "TurretsMustCenterBeforePacking", INI::parseBool,			NULL, offsetof( DeployStyleAIUpdateModuleData, m_turretsMustCenterBeforePacking ) },
			{ "ManualDeployAnimations",	INI::parseBool,							NULL, offsetof( DeployStyleAIUpdateModuleData, m_manualDeployAnimations ) },
			{ 0, 0, 0, 0 }
		};
		p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
class DeployStyleAIUpdate : public AIUpdateInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( DeployStyleAIUpdate, "DeployStyleAIUpdate"  )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( DeployStyleAIUpdate, DeployStyleAIUpdateModuleData )

private:

public:

	DeployStyleAIUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

 	virtual void aiDoCommand(const AICommandParms* parms);
	virtual Bool isIdle() const;
	virtual UpdateSleepTime update();

	UnsignedInt getUnpackTime()					const { return getDeployStyleAIUpdateModuleData()->m_unpackTime; }
	UnsignedInt getPackTime()						const { return getDeployStyleAIUpdateModuleData()->m_packTime; }
	Bool doTurretsFunctionOnlyWhenDeployed() const { return getDeployStyleAIUpdateModuleData()->m_turretsFunctionOnlyWhenDeployed; }
	Bool doTurretsHaveToCenterBeforePacking() const { return getDeployStyleAIUpdateModuleData()->m_turretsMustCenterBeforePacking; }
	void setMyState( DeployStateTypes StateID, Bool reverseDeploy = FALSE );

protected:

	DeployStateTypes				m_state;
	UnsignedInt							m_frameToWaitForDeploy;
};

#endif

