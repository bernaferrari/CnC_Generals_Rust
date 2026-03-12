// FILE: StructureBody.h //////////////////////////////////////////////////////////////////////////
// Author: Colin Day, November 2001
// Desc:	 Structure bodies are active bodies specifically for structures that are built
//				 and/or interactable (is that a world) with the player.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __STRUCTUREBODY_H_
#define __STRUCTUREBODY_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ActiveBody.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Object;

//-------------------------------------------------------------------------------------------------
class StructureBodyModuleData : public ActiveBodyModuleData 
{
public:

	StructureBodyModuleData(){}	

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    ActiveBodyModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
/** Structure body module */
//-------------------------------------------------------------------------------------------------
class StructureBody : public ActiveBody
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( StructureBody, "StructureBody" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( StructureBody, StructureBodyModuleData )

public:

	StructureBody( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	void setConstructorObject( Object *obj );
	ObjectID getConstructorObjectID( void ) { return m_constructorObjectID; }

protected:

	ObjectID m_constructorObjectID;					///< object that built this structure

};

#endif // __STRUCTUREBODY_H_

