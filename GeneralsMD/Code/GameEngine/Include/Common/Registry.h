// Registry.h
// Simple interface for storing/retreiving registry values
// Author: Matthew D. Campbell, December 2001

#pragma once

#ifndef __REGISTRY_H__
#define __REGISTRY_H__

#include <Common/AsciiString.h>

/**
 * Get a string from the original Generals Registry
 */
Bool GetStringFromGeneralsRegistry(AsciiString path, AsciiString key, AsciiString& val);
/**
	* Get a string from the registry
	*/
Bool GetStringFromRegistry(AsciiString path, AsciiString key, AsciiString& val);

/**
	* Get an unsigned int from the registry
	*/
Bool GetUnsignedIntFromRegistry(AsciiString path, AsciiString key, UnsignedInt& val);

AsciiString GetRegistryLanguage(void); // convenience function
AsciiString GetRegistryGameName(void); // convenience function
UnsignedInt GetRegistryVersion(void); // convenience function
UnsignedInt GetRegistryMapPackVersion(void); // convenience function

#endif // __REGISTRY_H__
