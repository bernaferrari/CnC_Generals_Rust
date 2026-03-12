// FILE: RadiusDecalUpdate.h /////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __RadiusDecalUpdate_H_
#define __RadiusDecalUpdate_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"
#include "GameClient/RadiusDecal.h"

//-------------------------------------------------------------------------------------------------
class RadiusDecalUpdateModuleData : public UpdateModuleData
{
public:
	//RadiusDecalTemplate	m_deliveryDecalTemplate;
	//Real								m_deliveryDecalRadius;

	RadiusDecalUpdateModuleData()
	{
		//m_deliveryDecalRadius = 0.0f;
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    UpdateModuleData::buildFieldParse(p);
		static const FieldParse dataFieldParse[] = 
		{
			//{ "DeliveryDecal",						RadiusDecalTemplate::parseRadiusDecalTemplate,	NULL, offsetof( RadiusDecalUpdateModuleData, m_deliveryDecalTemplate ) },
			//{ "DeliveryDecalRadius",			INI::parseReal,									NULL,	offsetof( RadiusDecalUpdateModuleData, m_deliveryDecalRadius ) },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}
};

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class RadiusDecalUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( RadiusDecalUpdate, "RadiusDecalUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( RadiusDecalUpdate, RadiusDecalUpdateModuleData )

public:

	RadiusDecalUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	//void createRadiusDecal( const Coord3D& pos );
	void createRadiusDecal( const RadiusDecalTemplate& tmpl, Real radius, const Coord3D& pos );
	void killWhenNoLongerAttacking(Bool v) { m_killWhenNoLongerAttacking = v; }
	void killRadiusDecal(); 

	virtual UpdateSleepTime update( void );

private:

	RadiusDecal m_deliveryDecal;
	Bool m_killWhenNoLongerAttacking;
};

#endif // __RadiusDecalUpdate_H_

