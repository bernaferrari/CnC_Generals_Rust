// FILE: INIWebpageURL.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Bryan Cleveland, November 2001
// Desc:   Parsing Webpage URL INI entries
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/INI.h"
#include "Common/Registry.h"
#include "GameNetwork/WOLBrowser/WebBrowser.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

///////////////////////////////////////////////////////////////////////////////////////////////////
// PRIVATE DATA ///////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

///////////////////////////////////////////////////////////////////////////////////////////////////
// PUBLIC FUNCTIONS ///////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

AsciiString encodeURL(AsciiString source)
{
	if (source.isEmpty())
	{
		return AsciiString::TheEmptyString;
	}

	AsciiString target;
	AsciiString allowedChars = "$-_.+!*'(),\\";
	const char *ptr = source.str();
	while (*ptr)
	{
		if (isalnum(*ptr) || allowedChars.find(*ptr))
		{
			target.concat(*ptr);
		}
		else
		{
			AsciiString tmp;
			target.concat('%');
			tmp.format("%2.2x", ((int)*ptr));
			target.concat(tmp);
		}
		++ptr;
	}

	return target;
}

//-------------------------------------------------------------------------------------------------
/** Parse Music entry */
//-------------------------------------------------------------------------------------------------
void INI::parseWebpageURLDefinition( INI* ini )
{
	AsciiString tag;
	WebBrowserURL *url;

	// read the name
	const char* c = ini->getNextToken();
	tag.set( c );

	if (TheWebBrowser != NULL)
	{
		url = TheWebBrowser->findURL(tag);

		if (url == NULL)
		{
			url = TheWebBrowser->makeNewURL(tag);
		}
	}

	// find existing item if present
//	track = TheAudio->Music->getTrack( name );
//	if( track == NULL )
//	{

		// allocate a new track
//		track = TheAudio->Music->newMusicTrack( name );

//	}  // end if

//	DEBUG_ASSERTCRASH( track, ("parseMusicTrackDefinition: Unable to allocate track '%s'\n",
//										 name.str()) );

	// parse the ini definition
	ini->initFromINI( url, url->getFieldParse() );

	if (url->m_url.startsWith("file://"))
	{
		char cwd[_MAX_PATH] = "\\";
		getcwd(cwd, _MAX_PATH);

		url->m_url.format("file://%s\\Data\\%s\\%s", encodeURL(cwd).str(), GetRegistryLanguage().str(), url->m_url.str()+7);
		DEBUG_LOG(("INI::parseWebpageURLDefinition() - converted URL to [%s]\n", url->m_url.str()));
	}
}  // end parseMusicTrackDefinition


