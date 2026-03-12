// FILE: INIMappedImage.cpp ///////////////////////////////////////////////////////////////////////
// Author: Colin Day, December 2001
// Desc:   Mapped image INI parsing
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "GameClient/Image.h"

///////////////////////////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
/** Parse mapped image entry */
//-------------------------------------------------------------------------------------------------
void INI::parseMappedImageDefinition( INI* ini )
{
	AsciiString name;

	// read the name
	const char* c = ini->getNextToken();
	name.set( c );	

	//
	// find existing item if present, note that we do not support overrides
	// in the images like we do in systems that are more "design" oriented, images
	// are assets as they are
	//
	if( !TheMappedImageCollection )
	{
		//We don't need it if we're in the builder... which doesn't have this.
		return;
	}
	Image *image = const_cast<Image*>(TheMappedImageCollection->findImageByName( name ));
	if(image)
		DEBUG_ASSERTCRASH(!image->getRawTextureData(), ("We are trying to parse over an existing image that contains a non-null rawTextureData, you should fix that"));

	if( image == NULL )
	{

		// image not found, create a new one
  	image = newInstance(Image);
		image->setName( name );
		TheMappedImageCollection->addImage(image);
		DEBUG_ASSERTCRASH( image, ("parseMappedImage: unable to allocate image for '%s'\n",
															name.str()) );

	}  // end if

	// parse the ini definition
	ini->initFromINI( image, image->getFieldParse());

}  // end parseMappedImage
