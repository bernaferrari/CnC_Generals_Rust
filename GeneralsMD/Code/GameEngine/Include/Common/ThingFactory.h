// FILE: ThingFactory.h ///////////////////////////////////////////////////////////////////////////
// Created: Colin Day, April 2001
// Desc:		This is how we go and make our things, we make our things, we make our things!	
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __THINGFACTORY_H_
#define __THINGFACTORY_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Lib/BaseType.h"

#include "Common/STLTypedefs.h"
#include "Common/SubsystemInterface.h"
#include "Common/GameMemory.h"
#include "Common/AsciiString.h"
#include "GameClient/Drawable.h"
#include "GameLogic/Object.h"

class ThingTemplate;
class Object;
class Drawable;
class INI;

typedef std::hash_map<AsciiString, ThingTemplate*, rts::hash<AsciiString>, rts::equal_to<AsciiString> > ThingTemplateHashMap;
typedef ThingTemplateHashMap::iterator ThingTemplateHashMapIt;
//-------------------------------------------------------------------------------------------------
/** Implementation of the thing manager interface singleton */
//-------------------------------------------------------------------------------------------------
class ThingFactory : public SubsystemInterface
{
public:

	ThingFactory( void );
	virtual ~ThingFactory( void );

	// From the subsystem interface =================================================================
	virtual void init( void );
	virtual void postProcessLoad( void );
	virtual void reset( void );
	virtual void update( void );
	//===============================================================================================

	/// create a new template with name 'name' and add to template list
	ThingTemplate *newTemplate( const AsciiString& name );

	// get the first template in our list 
	const ThingTemplate *firstTemplate( void ) { return m_firstTemplate; }

	/** 
		get a template given template database name. return null if not found.
		note, this is now substantially faster (does a hash-table lookup)
	*/
	const ThingTemplate *findTemplate( const AsciiString& name, Bool check = TRUE ) { return findTemplateInternal( name, check ); }

	/** 
		get a template given ID. return null if not found.
		note, this is not particularly fast (does a linear search).
	*/
	const ThingTemplate *findByTemplateID( UnsignedShort id );

	/** request a new object using the given template. 
		this will throw an exception on failure; it will never return null.
	*/
	Object *newObject( const ThingTemplate *tmplate, Team *team, ObjectStatusMaskType statusMask = OBJECT_STATUS_MASK_NONE );

	/** request a new drawable using the given template. 
		this will throw an exception on failure; it will never return null.
	*/
	Drawable *newDrawable(const ThingTemplate *tmplate, DrawableStatus statusBits = DRAWABLE_STATUS_NONE );

	static void parseObjectDefinition( INI* ini, const AsciiString& name, const AsciiString& reskinFrom );

private:

	/// free all template databse data
	void freeDatabase( void );

	void addTemplate( ThingTemplate *thing );		///< add the template to the DB

	/** 
		create a new template with name 'name', do *NOT* add to template, but instead
		add as m_override of template 'template' 

		Note, this is private, and should always remain so. It should never, ever, ever
		be called outside of ini parsing code, because if you do, you will tempted to make
		changes to the override at runtime, and end up with changes that can't be saved in savegames,
		since ThingTemplates aren't saved (and should not be). (srj)
	*/
	ThingTemplate* newOverride( ThingTemplate *thingTemplate );


	/** 
		This now does a search through the hashmap, and returns the associated thing template
		This should speed up load times dramatically, as we've been wasting lots of time in this
		O(N^2) insertion.
	
		NOTE: this is protected since it returns a NON-CONST template, and
		folks outside of the template system itself shouldn't get access...
	*/
	ThingTemplate *findTemplateInternal( const AsciiString& name, Bool check = TRUE );

	ThingTemplate					*m_firstTemplate;			///< head of linked list
	UnsignedShort					m_nextTemplateID;			///< next available ID for templates 

	ThingTemplateHashMap	m_templateHashMap;		///< all thing templates, for fast lookup.
		
};

// EXTERN /////////////////////////////////////////////////////////////////////////////////////////
extern ThingFactory *TheThingFactory;  ///< the template singleton


#endif // __THINGFACTORY_H_

