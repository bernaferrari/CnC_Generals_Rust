// FILE: FloatUpdate.h ////////////////////////////////////////////////////////////////////////////
// Author: Colin Day, May 2002
// Desc:   Floting on water update
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __FLOATUPDATE_H_
#define __FLOATUPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
struct FieldParse;

//-------------------------------------------------------------------------------------------------
class FloatUpdateModuleData: public UpdateModuleData
{

public:

	FloatUpdateModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);

	Bool m_enabled;							///< enabled

};

//-------------------------------------------------------------------------------------------------
class FloatUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( FloatUpdate, "FloatUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( FloatUpdate, FloatUpdateModuleData )

public:

	FloatUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	void setEnabled( Bool enabled ) { m_enabled = enabled; }  ///< enable/disable floating

	virtual UpdateSleepTime update();	///< Deciding whether or not to make new guys

protected:

	
	Bool m_enabled;			///< enabled

};

#endif  // end __FLOATUPDATE_H_
