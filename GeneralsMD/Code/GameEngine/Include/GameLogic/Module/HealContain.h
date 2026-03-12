// FILE: HealContain.h ////////////////////////////////////////////////////////////////////////////
// Author: Colin Day
// Desc:   Objects that are contained inside a heal contain ... get healed!  oh my!
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __HEALCONTAIN_H_
#define __HEALCONTAIN_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/OpenContain.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class HealContainModuleData : public OpenContainModuleData
{

public:

	HealContainModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);

	UnsignedInt m_framesForFullHeal;			///< time (in frames) something becomes fully healed

};

//-------------------------------------------------------------------------------------------------
/** The healing container ... bright white light ahhhhh goes here */
//-------------------------------------------------------------------------------------------------
class HealContain : public OpenContain
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( HealContain, "HealContain" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( HealContain, HealContainModuleData )
	
public:

	HealContain( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual UpdateSleepTime update();												///< called once per frame
	virtual Bool isHealContain() const { return true; } ///< true when container only contains units while healing (not a transport!)
	virtual Bool isTunnelContain() const { return FALSE; }

protected:

	Bool doHeal( Object *obj, UnsignedInt framesForFullHeal );		///< do the heal on an object

};

#endif  // end __HEALCONTAIN_H_
