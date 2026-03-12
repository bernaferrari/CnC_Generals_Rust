// FILE: OCLUpdate.h /////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, August2002
// Desc:   Update Spits out an OCL on a timer
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __OCL_UPDATE_H_
#define __OCL_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

class ObjectCreationList;

//-------------------------------------------------------------------------------------------------
class OCLUpdateModuleData : public UpdateModuleData
{
public:

	struct FactionOCLInfo
	{
		std::string									m_factionName;
		const ObjectCreationList *	m_ocl;
	};

	typedef std::list<FactionOCLInfo> FactionOCLList;

	const ObjectCreationList *	m_ocl;
	FactionOCLList							m_factionOCL;
	UnsignedInt									m_minDelay;
	UnsignedInt									m_maxDelay;
	Bool												m_isCreateAtEdge;				///< Otherwise, it is created on top of myself
	Bool												m_isFactionTriggered;		///< Faction has to be present before update will happen
	
	OCLUpdateModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);

private:

};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class OCLUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( OCLUpdate, "OCLUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( OCLUpdate, OCLUpdateModuleData )

public:

	OCLUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();

	Real getCountdownPercent() const; ///< goes from 0% to 100%
	UnsignedInt getRemainingFrames() const; ///< For feedback display
	void resetTimer(); ///< added for sabotage purposes.
	virtual DisabledMaskType getDisabledTypesToProcess() const { return DISABLEDMASK_ALL; }

protected:
	
	UnsignedInt			m_nextCreationFrame;
	UnsignedInt			m_timerStartedFrame;
	Bool						m_isFactionNeutral;
	Color						m_currentPlayerColor;

	Bool shouldCreate();
	void setNextCreationFrame();

};

#endif

