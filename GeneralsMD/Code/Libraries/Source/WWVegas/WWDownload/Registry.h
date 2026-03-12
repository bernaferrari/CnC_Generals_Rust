// Registry.h
// Simple interface for storing/retreiving registry values
// Author: Matthew D. Campbell, December 2001

#pragma once

#ifndef __WWDOWNLOAD_REGISTRY_H__
#define __WWDOWNLOAD_REGISTRY_H__

#include <string>

/**
	* Get a string from the registry
	*/
bool GetStringFromRegistry(std::string path, std::string key, std::string& val);

/**
	* Get an unsigned int from the registry
	*/
bool GetUnsignedIntFromRegistry(std::string path, std::string key, unsigned int& val);

/**
	* Store a string in the registry - returns true on success
	*/
bool SetStringInRegistry(std::string path, std::string key, std::string val);

/**
	* Store an unsigned int in the registry - returns true on success
	*/
bool SetUnsignedIntInRegistry(std::string path, std::string key, unsigned int val);

#endif // __WWDOWNLOAD_REGISTRY_H__
