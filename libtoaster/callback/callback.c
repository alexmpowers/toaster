#include "..\toaster.h"

#include <stdlib.h>
#include <string.h>

typedef struct toaster_signal_connection {
  char *signal;
  toaster_signal_callback_t callback;
  void *user_data;
} toaster_signal_connection_t;

struct toaster_signal_handler {
  toaster_signal_connection_t *connections;
  size_t connection_count;
  size_t connection_capacity;
};

typedef struct toaster_callback_snapshot {
  toaster_signal_callback_t callback;
  void *user_data;
} toaster_callback_snapshot_t;

static char *toaster_strdup(const char *text)
{
  size_t length;
  char *copy;

  if (!text)
    return NULL;

  length = strlen(text) + 1;
  copy = (char *)malloc(length);
  if (!copy)
    return NULL;

  memcpy(copy, text, length);
  return copy;
}

static bool ensure_connection_capacity(toaster_signal_handler_t *handler, size_t desired_count)
{
  toaster_signal_connection_t *new_connections;
  size_t new_capacity;

  if (!handler)
    return false;

  if (desired_count <= handler->connection_capacity)
    return true;

  new_capacity = handler->connection_capacity ? handler->connection_capacity * 2 : 8;
  while (new_capacity < desired_count)
    new_capacity *= 2;

  new_connections = (toaster_signal_connection_t *)realloc(
    handler->connections, new_capacity * sizeof(toaster_signal_connection_t));
  if (!new_connections)
    return false;

  handler->connections = new_connections;
  handler->connection_capacity = new_capacity;
  return true;
}

toaster_signal_handler_t *toaster_signal_handler_create(void)
{
  return (toaster_signal_handler_t *)calloc(1, sizeof(toaster_signal_handler_t));
}

void toaster_signal_handler_destroy(toaster_signal_handler_t *handler)
{
  size_t index;

  if (!handler)
    return;

  for (index = 0; index < handler->connection_count; ++index)
    free(handler->connections[index].signal);

  free(handler->connections);
  free(handler);
}

bool toaster_signal_handler_connect(toaster_signal_handler_t *handler, const char *signal,
                                    toaster_signal_callback_t callback, void *user_data)
{
  toaster_signal_connection_t *connection;
  char *signal_copy;

  if (!handler || !signal || !callback)
    return false;

  if (!ensure_connection_capacity(handler, handler->connection_count + 1))
    return false;

  signal_copy = toaster_strdup(signal);
  if (!signal_copy)
    return false;

  connection = &handler->connections[handler->connection_count++];
  connection->signal = signal_copy;
  connection->callback = callback;
  connection->user_data = user_data;
  return true;
}

bool toaster_signal_handler_disconnect(toaster_signal_handler_t *handler, const char *signal,
                                       toaster_signal_callback_t callback, void *user_data)
{
  size_t read_index;
  size_t write_index = 0;
  bool removed = false;

  if (!handler || !signal || !callback)
    return false;

  for (read_index = 0; read_index < handler->connection_count; ++read_index) {
    toaster_signal_connection_t *connection = &handler->connections[read_index];
    bool match = strcmp(connection->signal, signal) == 0 &&
                 connection->callback == callback && connection->user_data == user_data;

    if (match) {
      free(connection->signal);
      removed = true;
      continue;
    }

    if (write_index != read_index)
      handler->connections[write_index] = handler->connections[read_index];

    ++write_index;
  }

  handler->connection_count = write_index;
  return removed;
}

void toaster_signal_handler_emit(toaster_signal_handler_t *handler, const char *signal, void *param)
{
  size_t connection_index;
  size_t match_count = 0;
  size_t snapshot_index = 0;
  toaster_callback_snapshot_t *snapshots;

  if (!handler || !signal)
    return;

  for (connection_index = 0; connection_index < handler->connection_count; ++connection_index) {
    if (strcmp(handler->connections[connection_index].signal, signal) == 0)
      ++match_count;
  }

  if (match_count == 0)
    return;

  snapshots = (toaster_callback_snapshot_t *)calloc(match_count, sizeof(toaster_callback_snapshot_t));
  if (!snapshots) {
    for (connection_index = 0; connection_index < handler->connection_count; ++connection_index) {
      if (strcmp(handler->connections[connection_index].signal, signal) == 0)
        handler->connections[connection_index].callback(signal, param,
                                                        handler->connections[connection_index].user_data);
    }
    return;
  }

  for (connection_index = 0; connection_index < handler->connection_count; ++connection_index) {
    if (strcmp(handler->connections[connection_index].signal, signal) != 0)
      continue;

    snapshots[snapshot_index].callback = handler->connections[connection_index].callback;
    snapshots[snapshot_index].user_data = handler->connections[connection_index].user_data;
    ++snapshot_index;
  }

  for (connection_index = 0; connection_index < match_count; ++connection_index)
    snapshots[connection_index].callback(signal, param, snapshots[connection_index].user_data);

  free(snapshots);
}
